use aptos_logger::info;
use aptos_logger::warn;
use axum::response::IntoResponse;
use axum::Json;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::ffi::CString;
use std::process;
use std::ptr;
use std::sync::{Arc, Mutex};
use tikv_jemalloc_ctl::raw;
use tikv_jemallocator;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub struct HeapProfiler {
    mutex: Arc<Mutex<()>>,
}

const PROF_ACTIVE: &[u8] = b"prof.active\0";
const PROF_THREAD_ACTIVE_INIT: &[u8] = b"prof.thread_active_init\0";

pub static PROFILER: Lazy<HeapProfiler> = Lazy::new(|| {
    HeapProfiler::new()
});

#[derive(Deserialize, Serialize)]
pub struct ControlProfileRequest {
    start: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ControlProfileResponse {
    pub response: String,
}

pub async fn control_profiler(
    request: ControlProfileRequest,
) -> impl IntoResponse {
    match PROFILER.set_prof_active(request.start) {
        Ok(_) => Json(ControlProfileResponse {
            response: "success".to_string(),
        }),
        Err(e) => Json(ControlProfileResponse {
            response: e,
        }),
    }
}

fn dump_prof(path: &CString) {
    let option_name = CString::new("prof.dump").unwrap();
    let r = unsafe {
        tikv_jemalloc_sys::mallctl(
            option_name.as_ptr() as *const _,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            0,
        )
    };
    if r != 0 {
        panic!("jemalloc heap profiling dump failed: {}", r);
    }
}

impl HeapProfiler {
    pub fn new() -> Self {
        Self { mutex: Arc::new(Mutex::new(())) }
    }

    pub fn set_prof_active(&self, prof: bool) -> Result<(), String> {
        let _guard = self.mutex.lock().unwrap();
        if let Err(err) = unsafe { raw::write(PROF_ACTIVE, prof) } {
            warn!("jemalloc heap profiling active failed: {}", err);
            return Err(String::from(format!("jemalloc heap profiling active failed: {}", err)));
        }
        if let Err(err) = unsafe { raw::write(PROF_THREAD_ACTIVE_INIT, prof) } {
            warn!("jemalloc heap profiling thread_active_init failed: {}", err);
            return Err(String::from(format!(
                "jemalloc heap profiling thread_active_init failed: {}",
                err
            )));
        }
        if prof {
            info!("jemalloc heap profiling started");
        } else {
            info!("jemalloc heap profiling stopped");
        }
        Ok(())
    }

    fn get_prof_dump(&self, profile_file_name: &CString) {
        let _guard = self.mutex.lock().unwrap();
        dump_prof(profile_file_name);
    }

    pub fn dump_heap_profile(&self) -> String {
        let jeprofile_dir = env::var("JEPROFILE_DIR")
            .unwrap_or_else(|_| String::from("/home/jingyue/projects/gravity-sdk/jemalloc"));
        let profile_file_name = format!(
            "{}/jeheap_dump.{}.{}.{}.heap",
            jeprofile_dir,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            process::id(),
            rand::random::<u32>()
        );
        let profile_file_name1 = profile_file_name.clone();
        let profile_file_name_c =
            CString::new(profile_file_name).expect("Invalid characters in profile path");
        self.get_prof_dump(&profile_file_name_c);
        info!("the name is {:?}", profile_file_name_c);
        profile_file_name1
    }
}

// async fn start_profiling() -> impl Responder {
//     let profiler = HeapProfiler::new();
//     profiler.set_prof_active(true);
//     HttpResponse::Ok().body("Heap profiling started")
// }

// async fn stop_profiling() -> impl Responder {
//     let profiler = HeapProfiler::new();
//     profiler.set_prof_active(false);
//     HttpResponse::Ok().body("Heap profiling stopped")
// }

// async fn dump_profile() -> impl Responder {
//     let profiler = HeapProfiler::new();
//     let file_name = profiler.dump_heap_profile();
//     HttpResponse::Ok().body(format!("Heap profile dumped to {}", file_name))
// }

// async fn dump_profile_to_dot() -> impl Responder {
//     let profiler = HeapProfiler::new();
//     let file_name = profiler.dump_heap_profile_to_dot();
//     HttpResponse::Ok().body(format!("Heap profile dumped to dot format: {}", file_name))
// }

// #[actix_web::main]
// async fn main() -> std::io::Result<()> {
//     let profiler = Arc::new(Mutex::new(HeapProfiler::new()));

//     HttpServer::new(move || {
//         App::new()
//             .app_data(web::Data::new(profiler.clone()))
//             .route("/start", web::post().to(start_profiling))
//             .route("/stop", web::post().to(stop_profiling))
//             .route("/dump", web::post().to(dump_profile))
//             .route("/dump_dot", web::post().to(dump_profile_to_dot))
//     })
//     .bind("0.0.0.0:8080")?
//     .run()
//     .await
// }
// export _RJEM_MALLOC_CONF=prof:true,lg_prof_interval:30,lg_prof_sample:21,prof_prefix:/home/jingyue/projects/gravity-sdk/jemalloc
