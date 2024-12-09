use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
pub mod call;

pub enum Func {
    AddTxn(call::Call<Vec<u8>, ()>),
    TestInfo(call::Call<String, ()>),
}


pub struct CoExBridge {
    call: Arc<Mutex<HashMap<String, Func>>>,
}

lazy_static!(
    static ref coef_bridge: CoExBridge = CoExBridge::new();
);

pub fn get_coex_bridge() -> &'static CoExBridge {
    &coef_bridge
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

    fn take_func(&mut self, name: &str) -> Option<Func> {
        if let Ok(mut call) = self.call.lock() {
            call.remove(name)
        } else {
            panic!("Mutex lock failed");
        }
    }
}