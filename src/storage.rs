use super::models::Run;
use std::collections::HashMap;

#[derive(Default)]
pub struct Storage {
  store: HashMap<u16, Run>,
  count: u16,
}

impl Storage {
  pub fn new_run(&mut self) -> u16 {
    let count = self.count;
    self.count = count + 1;
    self.store.insert(count, Run::default());
    count
  }

  pub fn insert(&mut self, id: u16, value: usize) {
    self.store.get_mut(&id).unwrap().add_value(value);
  }

  pub fn run_ended(&mut self, id: u16) {
    self.store.get_mut(&id).unwrap().ended()
  }

  pub fn get_run(&self, id: &u16) -> Option<&Run> {
    self.store.get(id)
  }
}

#[test]
fn storage() {
  let mut storage = Storage::default();
  let id = storage.new_run();
  for value in 5..=6 {
    storage.insert(id, value);
  }
  let mut compare_run = Run::default();
  for value in 5..=6 {
    compare_run.add_value(value)
  }
  {
    let run = storage.get_run(&id).unwrap();
    assert_eq!(*run, compare_run);
  }
  storage.run_ended(id);
  compare_run.ended();
  {
    let run = storage.get_run(&id).unwrap();
    assert_eq!(*run, compare_run);
  }
}
