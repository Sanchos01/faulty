use super::storage::Storage;
use futures::{future::FutureExt, pin_mut, select};
use log::debug;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{self, Receiver, Sender};

type Db = Arc<RwLock<Storage>>;
const REQUESTS_START_COUNT: u16 = 5;
const REQUESTS_INCREASE_THRESHOLD: u16 = 50;
const EVERY_N_THRESHOLD: u8 = 20;

// Проблема:
// Необходимо дойти до максимального количества запросов к внешнему серверу и поддерживать его.
//
// Решение:
// Сначала экспоненциально увеличиваем максимальное количество запросов.
// При первых ошибках 429 - TOO MANY REQUESTS - также экспоненциально уменьшаем количество и дальнейший рост становится линейным.
// Последующие ошибки 429 изменяют коэффициент линейного роста (в меньшую сторону).
//
// Суть алгоритма:
// Изначально имеем некоторое максимальное количество запросов (REQUESTS_START_COUNT).
//
// Стратегия Double - при каждом успешном запросе увеличиваем максимальное количество запросов*.
// Первый запрос, вернувший 429, уменьшает максимальное количество запросов** и меняет стратегию на Single.
//
// Стратегия Single - при каждом успешном запросе увеличиваем максимальное количество запросов на 1.
// Запросы с ошибкой 429 и с версией First уменьшают максимальное количество запросов на 1.
// Первый запрос с ошибкой 429 и версией Second меняет стратегию на EveryN.
//
// Стратегия EveryN { count, threshold } - каждый успешный запрос увеличивает count на 1. Когда count равен threshold -
// увеличиваем максимальное количество запросов на 1, сбрасываем count.
// Запрос с ошибкой 429 - сбрасываем count в стратегии, увеличиваем threshold на 1 до предела EVERY_N_THRESHOLD.
//
//
// *  - Увеличиваем на два до тех пор, пока максимальное количество не будет больше REQUESTS_INCREASE_THRESHOLD,
//      далее увеличиваем на REQUESTS_INCREASE_THRESHOLD.
// ** - Уменьшаем на REQUESTS_INCREASE_THRESHOLD пока количество не будет меньше либо равно REQUESTS_INCREASE_THRESHOLD,
//      далее уменьшаем в два раза (до предела в единицу).

#[derive(Deserialize, Serialize)]
struct RunsResponse {
  value: usize,
}

pub fn new() -> Client {
  reqwest::ClientBuilder::new()
    .connect_timeout(Duration::from_secs(10))
    .timeout(Duration::from_secs(10))
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
  let mut receiver = RequestsInfo::new(id, client, db);
  receiver.start().await;
}

struct RequestsInfo {
  id: u16,
  rx: Receiver<RequestReturn>,
  tx: Sender<RequestReturn>,
  max: u16,
  started: u16,
  strategy: RequestStrategy,
  client: Client,
  db: Db,
  broadcast_tx: broadcast::Sender<StopSignal>,
}

impl std::fmt::Debug for RequestsInfo {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("RequestsInfo")
      .field("id", &self.id)
      .field("max", &self.max)
      .field("started", &self.started)
      .field("strategy", &self.strategy)
      .finish()
  }
}

impl std::ops::Drop for RequestsInfo {
  fn drop(&mut self) {
    let _ = self.broadcast_tx.send(StopSignal {});
  }
}

impl RequestsInfo {
  fn new(id: u16, client: Client, db: Db) -> Self {
    let (tx, rx) = mpsc::channel(100);
    let (broadcast_tx, _broadcast_rx) = broadcast::channel(1);
    RequestsInfo {
      id,
      rx,
      tx,
      max: REQUESTS_START_COUNT,
      started: 0,
      strategy: RequestStrategy::FastGrowth,
      client,
      db,
      broadcast_tx,
    }
  }

  async fn start(&mut self) {
    self.start_all_possible();
    debug!("requests_info start: {:?}", self);
    loop {
      if let Some(request_return) = self.rx.recv().await {
        self.started = self.started - 1;
        match request_return {
          RequestReturn {
            result: RequestResult::Error,
            ..
          } => self.start_one(),
          RequestReturn {
            result: RequestResult::Success { value },
            ..
          } => {
            self.db.write().unwrap().insert(self.id, value);
            self.update_count();
            self.start_all_possible();
            debug!("requests_info increase: {:?}", self);
          }
          RequestReturn {
            result: RequestResult::TooManyRequests,
            version,
          } => {
            if self.started <= self.max {
              self.reduce_by_strategy(version);
              self.start_all_possible();
            }
            debug!("requests_info reduce: {:?}", self);
          }
        }
      } else {
        break;
      }
    }
  }

  fn start_all_possible(&mut self) {
    for _ in self.started..self.max {
      self.start_one();
    }
  }

  fn start_one(&mut self) {
    self.started = self.started + 1;
    let tx = self.tx.clone();
    let client = self.client.clone();
    let version = self.request_version_by_strategy();
    let mut broadcast_rx = self.broadcast_tx.subscribe();
    tokio::spawn(async move {
      let f1 = handle_request(client, version, tx).fuse();
      let f2 = broadcast_rx.recv().fuse();
      pin_mut!(f1, f2);
      select! {
        _ = f1 => (),
        _ = f2 => ()
      };
    });
  }

  fn request_version_by_strategy(&self) -> RequestVersion {
    match self.strategy {
      RequestStrategy::FastGrowth => RequestVersion::First,
      _ => RequestVersion::Second,
    }
  }

  fn update_count(&mut self) {
    match self.strategy {
      RequestStrategy::FastGrowth => self.increase_max(),
      RequestStrategy::Single => self.increase_by_one(),
      RequestStrategy::EveryN { count, threshold } => {
        let new_count = count + 1;
        let new_strategy = if threshold == new_count {
          self.increase_by_one();
          RequestStrategy::EveryN {
            count: 0,
            threshold,
          }
        } else {
          RequestStrategy::EveryN {
            count: new_count,
            threshold,
          }
        };
        self.strategy = new_strategy;
      }
    }
  }

  fn reduce_by_strategy(&mut self, version: RequestVersion) {
    match self.strategy {
      RequestStrategy::FastGrowth => {
        self.strategy = RequestStrategy::Single;
        self.decrease_max();
      }
      RequestStrategy::Single => match version {
        RequestVersion::First => self.decrease_by_one(),
        RequestVersion::Second => {
          self.strategy = RequestStrategy::EveryN {
            count: 0,
            threshold: 2,
          };
          self.decrease_by_one();
        }
      },
      RequestStrategy::EveryN { threshold, .. } => {
        let new_strategy = if threshold == EVERY_N_THRESHOLD {
          RequestStrategy::EveryN {
            count: 0,
            threshold,
          }
        } else {
          RequestStrategy::EveryN {
            count: 0,
            threshold: threshold + 1,
          }
        };
        self.strategy = new_strategy;
        self.decrease_by_one()
      }
    }
  }

  fn increase_max(&mut self) {
    if self.max > REQUESTS_INCREASE_THRESHOLD {
      self.max = self.max + REQUESTS_INCREASE_THRESHOLD;
    } else {
      self.max = self.max * 2;
    }
  }

  fn increase_by_one(&mut self) {
    self.max = self.max + 1;
  }

  fn decrease_max(&mut self) {
    match self.max {
      x if x > REQUESTS_INCREASE_THRESHOLD => self.max = self.max - REQUESTS_INCREASE_THRESHOLD,
      1 => (),
      x => self.max = x / 2,
    }
  }

  fn decrease_by_one(&mut self) {
    match self.max {
      1 => (),
      x => self.max = x - 1,
    };
  }
}

async fn handle_request(client: Client, version: RequestVersion, mut tx: Sender<RequestReturn>) {
  let result = make_request(client).await;
  debug!("request result: {:?}", result);
  let request_return = RequestReturn { result, version };
  let _ = tx.send(request_return).await;
}

async fn make_request(client: Client) -> RequestResult {
  let host = host();
  let url = reqwest::Url::parse(host.as_str()).unwrap();
  if let Ok(response) = client.get(url).send().await {
    match response.status() {
      reqwest::StatusCode::OK => {
        if let Ok(body) = response.json::<RunsResponse>().await {
          RequestResult::Success { value: body.value }
        } else {
          // log parsing error?
          RequestResult::Error
        }
      }
      reqwest::StatusCode::TOO_MANY_REQUESTS => RequestResult::TooManyRequests,
      _status => RequestResult::Error,
    }
  } else {
    // log send request error?
    RequestResult::TooManyRequests
  }
}

#[derive(Debug)]
struct RequestReturn {
  result: RequestResult,
  version: RequestVersion,
}

#[derive(Debug)]
enum RequestResult {
  Error,
  TooManyRequests,
  Success { value: usize },
}

#[derive(Debug)]
enum RequestVersion {
  First,
  Second,
}

#[derive(Debug)]
enum RequestStrategy {
  FastGrowth,
  Single,
  EveryN { count: u8, threshold: u8 },
}

#[derive(Clone)]
struct StopSignal {}

// TODO: add mock, test
fn host() -> String {
  "http://faulty-server-htz-nbg1-1.wvservices.exchange:8080".into()
}
