//! Optional `Authorization` header forwarding from inbound JSON-RPC
//! requests to upstream RPC calls.
//!
//! When `HELIOS_FORWARD_AUTH=1` is set, the inbound HTTP middleware
//! captures the `Authorization` header into a task-local for the
//! duration of the request, and the outbound alloy layer attaches it
//! to every upstream call made while serving that request. Useful
//! when Helios sits behind a gateway that authenticates per-request
//! and the upstream backend expects the same credentials.
//!
//! # HTTP-only
//!
//! Forwarding only applies to HTTP requests. jsonrpsee dispatches
//! each WebSocket frame on a freshly spawned task, and Tokio
//! task-locals do not cross `tokio::spawn` boundaries — there is no
//! safe way to recover the upgrade-time `Authorization` header from
//! inside a per-frame handler. The caller in `jsonrpc::start`
//! therefore enables `http_only()` whenever this feature is on, so
//! WebSocket traffic isn't silently exposed to the upstream without
//! credentials.

use std::env;
use std::pin::Pin;
use std::task::{Context, Poll};

use alloy::rpc::json_rpc::{RequestPacket, ResponsePacket};
use alloy::transports::TransportError;
use tower::{Layer, Service};

// jsonrpsee 0.19 uses `http` 0.2 on its server; alloy 1.0 uses `http`
// 1.x on its transport. We parse the captured bytes into the alloy
// (1.x) `HeaderValue` once on capture, then clone it cheaply on each
// upstream call.

const ENV_VAR_NAME: &str = "HELIOS_FORWARD_AUTH";
const ENABLED_VALUE: &str = "1";

tokio::task_local! {
    /// `Authorization` header captured on the inbound request,
    /// scoped to that request's handler tree. Read by
    /// [`AuthForwardLayer`] when constructing upstream calls.
    pub static FORWARDED_AUTH: Option<http::HeaderValue>;
}

/// Returns `true` if `HELIOS_FORWARD_AUTH=1` is set in the environment.
///
/// Checked at startup; the inbound capture middleware is only
/// installed when this returns `true`.
pub fn is_enabled() -> bool {
    matches!(env::var(ENV_VAR_NAME).as_deref(), Ok(v) if v == ENABLED_VALUE)
}

// ---------- Inbound: capture `Authorization` into a task-local ----------

/// Tower layer that captures the inbound `Authorization` header into
/// [`FORWARDED_AUTH`] for the duration of the wrapped service call.
#[derive(Clone, Copy, Debug, Default)]
pub struct AuthCaptureLayer;

impl<S> Layer<S> for AuthCaptureLayer {
    type Service = AuthCaptureService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthCaptureService { inner }
    }
}

#[derive(Clone, Debug)]
pub struct AuthCaptureService<S> {
    inner: S,
}

impl<S, ReqBody> Service<http02::Request<ReqBody>> for AuthCaptureService<S>
where
    S: Service<http02::Request<ReqBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<S::Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http02::Request<ReqBody>) -> Self::Future {
        let auth = req
            .headers()
            .get(http02::header::AUTHORIZATION)
            .and_then(|v| http::HeaderValue::from_bytes(v.as_bytes()).ok());
        let mut inner = self.inner.clone();
        Box::pin(async move { FORWARDED_AUTH.scope(auth, inner.call(req)).await })
    }
}

// ---------- Outbound: attach the captured header to upstream calls ----------

/// Alloy tower layer that, when [`FORWARDED_AUTH`] is set, attaches
/// the captured `Authorization` header to each outbound
/// [`RequestPacket`] via the request-meta `HeaderMap` extension.
/// The default `Http<reqwest::Client>` transport reads this extension
/// and forwards the headers on the upstream HTTP request.
#[derive(Clone, Copy, Debug, Default)]
pub struct AuthForwardLayer;

impl<S> Layer<S> for AuthForwardLayer {
    type Service = AuthForwardService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthForwardService { inner }
    }
}

#[derive(Clone, Debug)]
pub struct AuthForwardService<S> {
    inner: S,
}

impl<S> Service<RequestPacket> for AuthForwardService<S>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>,
{
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: RequestPacket) -> Self::Future {
        let _ = FORWARDED_AUTH.try_with(|auth| {
            let Some(value) = auth.as_ref() else { return };
            let serialized = match &mut req {
                RequestPacket::Single(s) => std::slice::from_mut(s),
                RequestPacket::Batch(b) => b.as_mut_slice(),
            };
            for s in serialized {
                let mut headers = http::HeaderMap::with_capacity(1);
                headers.insert(http::header::AUTHORIZATION, value.clone());
                s.meta_mut().extensions_mut().insert(headers);
            }
        });
        self.inner.call(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::rpc::json_rpc::Request;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    /// Inner service that records whether the task-local was set when
    /// it was called, returning a unit response.
    #[derive(Clone, Default)]
    struct CaptureProbe(Arc<Mutex<Option<Option<http::HeaderValue>>>>);

    impl<B: Send + 'static> Service<http02::Request<B>> for CaptureProbe {
        type Response = http02::Response<()>;
        type Error = std::convert::Infallible;
        type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, _: http02::Request<B>) -> Self::Future {
            let probe = self.0.clone();
            Box::pin(async move {
                let observed = FORWARDED_AUTH.try_with(|a| a.clone()).ok();
                *probe.lock().unwrap() = observed;
                Ok(http02::Response::new(()))
            })
        }
    }

    #[tokio::test]
    async fn capture_sets_task_local_when_header_present() {
        let probe = CaptureProbe::default();
        let svc = AuthCaptureLayer.layer(probe.clone());

        let req = http02::Request::builder()
            .header(http02::header::AUTHORIZATION, "Bearer token-xyz")
            .body(())
            .unwrap();
        svc.oneshot(req).await.unwrap();

        let expected = http::HeaderValue::from_static("Bearer token-xyz");
        assert_eq!(*probe.0.lock().unwrap(), Some(Some(expected)));
    }

    #[tokio::test]
    async fn capture_records_none_when_header_absent() {
        let probe = CaptureProbe::default();
        let svc = AuthCaptureLayer.layer(probe.clone());

        let req = http02::Request::builder().body(()).unwrap();
        svc.oneshot(req).await.unwrap();

        assert_eq!(*probe.0.lock().unwrap(), Some(None));
    }

    /// Inner alloy service that records the request-meta `HeaderMap`
    /// extension on each call.
    #[derive(Clone, Default)]
    struct PacketProbe(Arc<Mutex<Option<http::HeaderMap>>>);

    impl Service<RequestPacket> for PacketProbe {
        type Response = ResponsePacket;
        type Error = TransportError;
        type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: RequestPacket) -> Self::Future {
            let observed = match &req {
                RequestPacket::Single(s) => s.meta().extensions().get::<http::HeaderMap>().cloned(),
                RequestPacket::Batch(_) => None,
            };
            *self.0.lock().unwrap() = observed;
            Box::pin(async move {
                Err::<ResponsePacket, _>(alloy::transports::TransportErrorKind::custom_str("probe"))
            })
        }
    }

    fn make_packet() -> RequestPacket {
        let req: Request<()> = Request::new("eth_blockNumber", 1u64.into(), ());
        RequestPacket::Single(req.serialize().unwrap())
    }

    #[tokio::test]
    async fn forward_injects_header_when_task_local_set() {
        let probe = PacketProbe::default();
        let mut svc = AuthForwardLayer.layer(probe.clone());

        let auth = http::HeaderValue::from_static("Bearer token-xyz");
        FORWARDED_AUTH
            .scope(Some(auth.clone()), async {
                let _ = svc.call(make_packet()).await;
            })
            .await;

        let observed = probe.0.lock().unwrap().clone().expect("HeaderMap injected");
        assert_eq!(observed.get(http::header::AUTHORIZATION), Some(&auth));
    }

    #[tokio::test]
    async fn forward_injects_header_into_each_request_in_batch() {
        #[derive(Clone, Default)]
        struct BatchProbe(Arc<Mutex<Vec<Option<http::HeaderMap>>>>);
        impl Service<RequestPacket> for BatchProbe {
            type Response = ResponsePacket;
            type Error = TransportError;
            type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;
            fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }
            fn call(&mut self, req: RequestPacket) -> Self::Future {
                let observed: Vec<_> = req
                    .requests()
                    .iter()
                    .map(|s| s.meta().extensions().get::<http::HeaderMap>().cloned())
                    .collect();
                *self.0.lock().unwrap() = observed;
                Box::pin(async move {
                    Err::<ResponsePacket, _>(alloy::transports::TransportErrorKind::custom_str("probe"))
                })
            }
        }

        let probe = BatchProbe::default();
        let mut svc = AuthForwardLayer.layer(probe.clone());

        let mut packet = make_packet();
        packet.push(Request::<()>::new("eth_chainId", 2u64.into(), ()).serialize().unwrap());

        let auth = http::HeaderValue::from_static("Bearer batch-token");
        FORWARDED_AUTH
            .scope(Some(auth.clone()), async {
                let _ = svc.call(packet).await;
            })
            .await;

        let observed = probe.0.lock().unwrap().clone();
        assert_eq!(observed.len(), 2);
        for hm in observed {
            assert_eq!(hm.unwrap().get(http::header::AUTHORIZATION), Some(&auth));
        }
    }

    #[tokio::test]
    async fn forward_is_noop_when_task_local_unset() {
        let probe = PacketProbe::default();
        let mut svc = AuthForwardLayer.layer(probe.clone());

        let _ = svc.call(make_packet()).await;
        assert!(probe.0.lock().unwrap().is_none());
    }

    #[test]
    #[serial_test::serial]
    fn is_enabled_reads_env_var() {
        let prior = env::var(ENV_VAR_NAME).ok();

        unsafe { env::set_var(ENV_VAR_NAME, ENABLED_VALUE) };
        assert!(is_enabled());

        unsafe { env::set_var(ENV_VAR_NAME, "0") };
        assert!(!is_enabled());

        unsafe { env::remove_var(ENV_VAR_NAME) };
        assert!(!is_enabled());

        match prior {
            Some(v) => unsafe { env::set_var(ENV_VAR_NAME, v) },
            None => unsafe { env::remove_var(ENV_VAR_NAME) },
        }
    }
}
