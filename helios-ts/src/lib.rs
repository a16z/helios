#![allow(unexpected_cfgs)]

use eyre::Result;
use wasm_bindgen::JsError;

pub mod ethereum;
pub mod opstack;
pub mod storage;

#[allow(unused_macros)]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

fn map_err<T>(val: Result<T>) -> Result<T, JsError> {
    val.map_err(|err| JsError::new(&err.to_string()))
}
