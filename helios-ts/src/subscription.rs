use std::marker::PhantomData;
use wasm_bindgen::prelude::*;
use web_sys::js_sys::Function;

use futures_util::future::{AbortHandle, Abortable};

use helios_common::network_spec::NetworkSpec;
use helios_common::types::SubEventRx;

pub struct Subscription<N: NetworkSpec> {
    abort_handle: AbortHandle,
    _phantom: PhantomData<N>,
}

impl<N: NetworkSpec> Subscription<N> {
    pub fn new(abort_handle: AbortHandle) -> Self {
        Self {
            abort_handle,
            _phantom: PhantomData,
        }
    }

    pub fn spawn_listener(id: String, mut rx: SubEventRx<N>, callback: Function) -> Self {
        let (abort_handle, abort_registration) = AbortHandle::new_pair();

        let listener_id = id.clone();
        let future = async move {
            while let Ok(msg) = rx.recv().await {
                if let Ok(data) = serde_wasm_bindgen::to_value(&msg) {
                    let _ = callback.call2(&JsValue::NULL, &data, &JsValue::from_str(&listener_id));
                }
            }
        };

        wasm_bindgen_futures::spawn_local(async move {
            let _ = Abortable::new(future, abort_registration).await;
        });

        Self::new(abort_handle)
    }

    pub fn abort(&self) {
        self.abort_handle.abort();
    }
}

impl<N: NetworkSpec> Drop for Subscription<N> {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}
