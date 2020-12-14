use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use warp::Filter;

mod handlers;
mod models;
mod requester;
mod storage;

#[tokio::main]
async fn main() {
  std::env::set_var("RUST_LOG", "info");
  env_logger::init();

  let is_running = Arc::new(AtomicBool::new(false));
  // init storage
  let storage = Arc::new(RwLock::new(storage::Storage::default()));
  // init http client
  let client = requester::new();

  // POST /runs {seconds: 30} => {id: 7}
  let s = storage.clone();
  let r = is_running.clone();
  let start_run = warp::post()
    .and(warp::path("runs"))
    .and(warp::path::end())
    .and(warp::body::content_length_limit(1024))
    .and(warp::body::json())
    .and(warp::any().map(move || s.clone()))
    .and(warp::any().map(move || client.clone()))
    .and(warp::any().map(move || r.clone()))
    .and_then(handlers::start_run);
  // GET /runs/:id => {status: "IN_PROGRESS", successful_responses_count: 17, sum: 712}
  let s = storage.clone();
  let get_run = warp::get()
    .and(warp::path!("runs" / u16))
    .and(warp::path::end())
    .and(warp::any().map(move || s.clone()))
    .and_then(handlers::get_run);
  let routes = start_run.or(get_run);

  warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}
