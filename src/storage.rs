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
