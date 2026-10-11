use std::collections::{HashMap, VecDeque};

use crate::mongo::{command_name::CommandName, expectations::Expectation};

#[derive(Default)]
pub(crate) struct State {
    pub expectations: HashMap<CommandName, VecDeque<Expectation>>,
}

impl State {
    pub fn get_expetation(&mut self, command_name: CommandName) -> Option<Expectation> {
        self.expectations
            .get_mut(&command_name)
            .and_then(|e| e.pop_front())
    }

    pub fn insert_expectation(&mut self, command_name: CommandName, expectation: Expectation) {
        self.expectations
            .entry(command_name)
            .or_default()
            .push_back(expectation);
    }
}
