use crate::{InspectCommand, LogsCommand, RmCommand, RunCommand, StopCommand};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::runtime::Runtime;

/// Implementation of the Docker client API using the docker cli.
pub struct Docker {
    tokio_runtime: Rc<RefCell<Runtime>>,
}

impl Docker {
    pub fn new() -> Self {
        let tokio_runtime = Runtime::new().unwrap();
        Self {
            tokio_runtime: Rc::new(RefCell::new(tokio_runtime)),
        }
    }

    /// Docker run command
    pub fn run(&self) -> RunCommand {
        RunCommand::new(self.tokio_runtime.clone())
    }

    /// Docker logs command
    pub fn logs(&self) -> LogsCommand {
        LogsCommand::new(self.tokio_runtime.clone())
    }

    /// Docker inspect command
    pub fn inspect(&self) -> InspectCommand {
        InspectCommand::new(self.tokio_runtime.clone())
    }

    /// Docker rm command
    pub fn rm(&self) -> RmCommand {
        RmCommand::new(self.tokio_runtime.clone())
    }

    /// Docker stop command
    pub fn stop(&self) -> StopCommand {
        StopCommand::new(self.tokio_runtime.clone())
    }
}
