use std::{cell::Cell, marker::PhantomData, rc::Rc};

use helios_common::{network_spec::NetworkSpec, types::SubEventRx};
use wasm_bindgen::prelude::*;
use web_sys::js_sys::Function;

pub struct Subscription<N: NetworkSpec> {
    id: String,
    active: Rc<Cell<bool>>,
    _phantom: PhantomData<N>,
}

impl<N: NetworkSpec> Subscription<N> {
    pub fn new(id: String) -> Self {
        Self {
            id,
            active: Rc::new(Cell::new(true)),
            _phantom: PhantomData,
        }
    }

    pub async fn listen(&self, mut rx: SubEventRx<N>, callback: Function) {
        let id = self.id.clone();
        let active = self.active.clone();

        wasm_bindgen_futures::spawn_local(async move {
            while let Ok(msg) = rx.recv().await {
                if !active.get() {
                    break;
                }

                if let Ok(data) = serde_wasm_bindgen::to_value(&msg) {
                    let _ = callback.call2(&JsValue::NULL, &data, &JsValue::from_str(&id));
                }
            }
        });
    }
}

impl<N: NetworkSpec> Drop for Subscription<N> {
    fn drop(&mut self) {
        self.active.set(false);
    }
}
