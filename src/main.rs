use actix_web::{error, get, post, web, App, Error, HttpResponse, HttpServer, Responder, Result};
use actix_web::middleware::Logger;
use env_logger::Env;
use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use std::time::SystemTime;

use serde::Deserialize;


struct Global {
    lock_map: Arc<RwLock<HashMap<String, Mutex<i32>>>>,         // Key: the name of the job, Value: Mutex lock
    job_record_map: Arc<RwLock<HashMap<String, SystemTime>>>,   // Key: the name of the job, Value: the time when job started the most recently
}

#[derive(Deserialize)]
struct LockInfo {
    lock_name: String,
}


#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn lock(lock_info: web::Json<LockInfo>, data: web::Data<Global>) -> Result<String, Error> {
    // Assume that the lock_mutex already exists
    let lock_map = data.lock_map.read().expect("RwLock poisoned");
    if let Some(lock_mutex) = lock_map.get(&(lock_info.lock_name)) {
        let mut lock_mutex = lock_mutex.lock().expect("Mutex poisoned");
        if lock_mutex.is_positive() {
            return Err(error::ErrorBadRequest("Unable to get lock"));
        }
        *lock_mutex += 1;
        return Ok(format!("Lock success"));
    }
     // If we got this far, the element doesn't exist

    // Get rid of our read lock and switch to a write lock
    // You want to minimize the time we hold the writer lock
    drop(lock_map);
    let mut lock_map = data.lock_map.write().expect("RwLock poisoned");

    // We use HashMap::entry to handle the case where another thread 
    // inserted the same key while where were unlocked.
    lock_map.entry(lock_info.lock_name.to_string()).or_insert_with(|| Mutex::new(1));

    Ok(format!("Lock success"))
}

async fn unlock(lock_info: web::Json<LockInfo>, data: web::Data<Global>) -> Result<String> {
    let lock_map = data.lock_map.read().expect("RwLock poisoned");
    if let Some(lock_mutex) = lock_map.get(&(lock_info.lock_name)) {
        let mut lock_mutex = lock_mutex.lock().expect("Mutex poisoned");
        if lock_mutex.is_negative() || *lock_mutex == 0  {
            return Err(error::ErrorBadRequest("Unable to get lock"));
        }
        *lock_mutex -= 1;
        return Ok(format!("Unlock success"));
    }

    Err(error::ErrorBadRequest("Lock name does not exist"))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Note: web::Data created _outside_ HttpServer::new closure
    let global_data = web::Data::new(Global {
        lock_map: Arc::new(RwLock::new(HashMap::new())),
        job_record_map: Arc::new(RwLock::new(HashMap::new())),
    });

    env_logger::init_from_env(Env::default().default_filter_or("info"));

    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .wrap(Logger::new("%a %{User-Agent}i"))
            .app_data(global_data.clone()) // <- register the created data
            .service(hello)
            .service(echo)
            .route("/lock", web::post().to(lock))
            .route("/unlock", web::post().to(unlock))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}