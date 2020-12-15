use serde::Serialize;
use warp::reply::Response;

#[derive(Debug, Serialize, Clone, Default, PartialEq)]
pub struct Run {
  status: RunStatus,
  successful_responses_count: u16,
  sum: usize,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum RunStatus {
  InProgress,
  Finished,
}

impl Default for RunStatus {
  fn default() -> Self {
    RunStatus::InProgress
  }
}

impl Run {
  pub fn add_value(&mut self, value: usize) {
    self.successful_responses_count = self.successful_responses_count + 1;
    self.sum = self.sum + value;
  }

  pub fn ended(&mut self) {
    self.status = RunStatus::Finished;
  }
}

impl warp::reply::Reply for Run {
  fn into_response(self) -> Response {
    let body = serde_json::to_string(&self).unwrap();
    http::Response::builder()
      .status(200)
      .header("Content-Type", "application/json")
      .body(body.into())
      .unwrap()
  }
}

#[test]
fn run() {
  let mut run = Run::default();
  let r = Run {
    status: RunStatus::InProgress,
    successful_responses_count: 0,
    sum: 0,
  };
  assert_eq!(run, r);
  run.add_value(10);
  run.add_value(15);
  let r = Run {
    status: RunStatus::InProgress,
    successful_responses_count: 2,
    sum: 25,
  };
  assert_eq!(run, r);
  run.ended();
  let r = Run {
    status: RunStatus::Finished,
    successful_responses_count: 2,
    sum: 25,
  };
  assert_eq!(run, r);
}
