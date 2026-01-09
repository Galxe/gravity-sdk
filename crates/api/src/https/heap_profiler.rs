use axum::{response::IntoResponse, Json};
use gaptos::aptos_logger::{info, warn};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    env,
    sync::{Arc, Mutex},
};
use tikv_jemalloc_ctl::raw;

#[allow(dead_code)]
pub struct HeapProfiler {
    mutex: Arc<Mutex<()>>,
}

#[allow(dead_code)]
const PROF_ACTIVE: &[u8] = b"prof.active\0";
#[allow(dead_code)]
const PROF_THREAD_ACTIVE_INIT: &[u8] = b"prof.thread_active_init\0";

#[allow(dead_code)]
pub static PROFILER: Lazy<HeapProfiler> = Lazy::new(HeapProfiler::new);

#[derive(Deserialize, Serialize)]
pub struct ControlProfileRequest {
    enable: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ControlProfileResponse {
    pub response: String,
}

/// User should use binary with feature api/jemalloc-profiling enabled.
/// This feature can be enabled by ```Cargo build --features api/jemalloc-profiling```
pub async fn control_profiler(_request: ControlProfileRequest) -> impl IntoResponse {
    #[cfg(feature = "jemalloc-profiling")]
    match PROFILER.set_prof_active(_request.enable) {
        Ok(_) => Json(ControlProfileResponse { response: "success".to_string() }),
        Err(e) => Json(ControlProfileResponse { response: e }),
    }
    #[cfg(not(feature = "jemalloc-profiling"))]
    Json(ControlProfileResponse { response: "jemalloc profiling is not enabled".to_string() })
}

impl HeapProfiler {
    pub fn new() -> Self {
        Self { mutex: Arc::new(Mutex::new(())) }
    }

    #[allow(dead_code)]
    pub fn set_prof_active(&self, prof: bool) -> Result<(), String> {
        let _guard = self.mutex.lock().unwrap();
        if let Err(err) = unsafe { raw::write(PROF_ACTIVE, prof) } {
            let err = format!("jemalloc heap profiling active failed: {err}");
            warn!("{}", err);
            return Err(err);
        }
        if let Err(err) = unsafe { raw::write(PROF_THREAD_ACTIVE_INIT, prof) } {
            let err = format!("jemalloc heap profiling thread_active_init failed: {err}");
            warn!("{}", err);
            return Err(err);
        }
        if prof {
            info!("jemalloc heap profiling started");
        } else {
            info!("jemalloc heap profiling stopped");
        }
        Ok(())
    }
}
