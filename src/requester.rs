use super::storage::Storage;
use futures::{future::FutureExt, pin_mut, select};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, RwLock};
use std::time::Duration;

type Db = Arc<RwLock<Storage>>;

#[derive(Deserialize, Serialize)]
struct RunsResponse {
  value: usize,
}

pub fn new() -> Client {
  reqwest::ClientBuilder::new()
    .pool_max_idle_per_host(1)
    .connect_timeout(Duration::from_secs(30))
    .timeout(Duration::from_secs(30))
    .build()
    .unwrap()
}

pub async fn run(client: Client, id: u16, seconds: u16, is_running: Arc<AtomicBool>, db: Db) {
  let db2 = db.clone();
  let f1 = loop_requests(client, id, db).fuse();
  let f2 = tokio::time::delay_for(Duration::from_secs(seconds as u64)).fuse();
  pin_mut!(f1, f2);
  select! {
    () = f1 => (),
    () = f2 => {
      db2.write().unwrap().run_ended(id);
      is_running.store(false, Relaxed);
    }
  }
}

async fn loop_requests(client: Client, id: u16, db: Db) {
  loop {
    let db = db.clone();
    let host = host();
    let url = reqwest::Url::parse(host.as_str()).unwrap();
    if let Ok(response) = client.get(url).send().await {
      if response.status() == reqwest::StatusCode::OK {
        if let Ok(body) = response.json::<RunsResponse>().await {
          db.write().unwrap().insert(id, body.value);
        }
      }
    }
  }
}

// TODO: add mock, test
fn host() -> String {
  "http://faulty-server-htz-nbg1-1.wvservices.exchange:8080".into()
}
