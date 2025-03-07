use aptos_logger::info;
use aptos_logger::warn;
use axum::response::IntoResponse;
use axum::Json;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::sync::{Arc, Mutex};
use tikv_jemalloc_ctl::raw;

pub struct HeapProfiler {
    mutex: Arc<Mutex<()>>,
}

const PROF_ACTIVE: &[u8] = b"prof.active\0";
const PROF_THREAD_ACTIVE_INIT: &[u8] = b"prof.thread_active_init\0";

pub static PROFILER: Lazy<HeapProfiler> = Lazy::new(|| HeapProfiler::new());

#[derive(Deserialize, Serialize)]
pub struct ControlProfileRequest {
    start: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ControlProfileResponse {
    pub response: String,
}

pub async fn control_profiler(request: ControlProfileRequest) -> impl IntoResponse {
    #[cfg(feature = "jemalloc-profiling")]
    match PROFILER.set_prof_active(request.start) {
        Ok(_) => Json(ControlProfileResponse { response: "success".to_string() }),
        Err(e) => Json(ControlProfileResponse { response: e }),
    }
    #[cfg(not(feature = "jemalloc-profiling"))]
    Json(ControlProfileResponse {
        response: "jemalloc profiling is not enabled".to_string(),
    })
}

impl HeapProfiler {
    pub fn new() -> Self {
        Self { mutex: Arc::new(Mutex::new(())) }
    }

    pub fn set_prof_active(&self, prof: bool) -> Result<(), String> {
        let _guard = self.mutex.lock().unwrap();
        if let Err(err) = unsafe { raw::write(PROF_ACTIVE, prof) } {
            let err = String::from(format!("jemalloc heap profiling active failed: {}", err));
            warn!("{}", err);
            return Err(err);
        }
        if let Err(err) = unsafe { raw::write(PROF_THREAD_ACTIVE_INIT, prof) } {
            let err =
                String::from(format!("jemalloc heap profiling thread_active_init failed: {}", err));
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
