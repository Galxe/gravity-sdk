use std::collections::HashMap;
use std::sync::{Arc, Mutex};
pub mod call;

pub enum Func {
    AddTxn(call::Call<Vec<u8>, ()>),
    TestInfo(call::Call<String, ()>),
}


pub struct CoExBridge {
    call: Arc<Mutex<HashMap<String, Func>>>,
}


use once_cell::sync::Lazy;

pub static COEX_BRIDGE: Lazy<CoExBridge> = Lazy::new(|| CoExBridge::new());

pub fn get_coex_bridge() -> &'static CoExBridge {
    &COEX_BRIDGE
}

impl CoExBridge {
    pub fn new() -> Self {
        Self {
            call: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register(&self, name: String, func: Func) {
        if let Ok(mut call) = self.call.lock() {
            if call.contains_key(&name) {
                panic!("Function already registered");
            }
            call.insert(name, func);
        } else {
            panic!("Mutex lock failed");
        }
    }

    pub fn take_func(&self, name: &str) -> Option<Func> {
        if let Ok(mut call) = self.call.lock() {
            call.remove(name)
        } else {
            panic!("Mutex lock failed");
        }
    }
}