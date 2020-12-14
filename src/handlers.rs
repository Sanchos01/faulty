use crate::requester;

use super::storage::Storage;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use warp::reply::Response;
use warp::Reply;

type Db = Arc<RwLock<Storage>>;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRunRequest {
  seconds: u16,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CreateRunResponse {
  id: u16,
}

impl warp::reply::Reply for CreateRunResponse {
  fn into_response(self) -> Response {
    let body = serde_json::to_string(&self).unwrap();
    http::Response::builder()
      .status(200)
      .header("Content-Type", "application/json")
      .body(body.into())
      .unwrap()
  }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BadRequest<'a> {
  error: &'a str,
}

impl warp::reply::Reply for BadRequest<'_> {
  fn into_response(self) -> Response {
    let body = serde_json::to_string(&self).unwrap();
    http::Response::builder()
      .status(400)
      .header("Content-Type", "application/json")
      .body(body.into())
      .unwrap()
  }
}

pub async fn start_run(
  body: CreateRunRequest,
  db: Db,
  client: Client,
  is_running: Arc<AtomicBool>,
) -> Result<impl warp::Reply, Infallible> {
  let running = is_running.compare_and_swap(false, true, Ordering::Relaxed);
  if !running {
    let id = db.write().unwrap().new_run();
    tokio::spawn(async move { requester::run(client, id, body.seconds, is_running, db).await });
    let response = CreateRunResponse { id };
    Ok(response.into_response())
  } else {
    let response = BadRequest {
      error: "'run' already started",
    };
    Ok(response.into_response())
  }
}

pub async fn get_run(id: u16, db: Db) -> Result<impl warp::Reply, Infallible> {
  if let Some(run) = db.read().unwrap().get_run(&id) {
    Ok(run.clone().into_response())
  } else {
    let response = BadRequest {
      error: "'run' not exists",
    };
    Ok(response.into_response())
  }
}
