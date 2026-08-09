use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use polaris_core::db::open_database;
use polaris_core::engine::Engine;
use polaris_core::status::StatusSnapshot;

use crate::contracts::CommandError;

pub struct DesktopState {
    engine: Mutex<Engine>,
}

impl DesktopState {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CommandError> {
        let connection = open_database(path).map_err(CommandError::core)?;
        Ok(Self {
            engine: Mutex::new(Engine::new(connection)),
        })
    }

    fn engine(&self) -> Result<MutexGuard<'_, Engine>, CommandError> {
        self.engine
            .lock()
            .map_err(|_| CommandError::state("Polaris 引擎暂时不可用，请重试"))
    }

    pub fn status(&self) -> Result<StatusSnapshot, CommandError> {
        self.engine()?.status_snapshot().map_err(CommandError::core)
    }
}
