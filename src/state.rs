use std::sync::Arc;

use crate::config::Config;

pub struct State {
    pub config: Config,
}

pub type AppState = Arc<State>;

impl State {
    pub fn new(config: Config) -> AppState {
        Arc::new(State { config })
    }
}
