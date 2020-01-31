use crate::{Image, InspectCommand, LogsCommand, RmCommand, RunCommand, StopCommand};

/// Implementation of the Docker client API using the docker cli.
pub struct Docker;

impl Docker {
    /// Docker run command
    pub fn run<I: Image>(image: I) -> RunCommand<I> {
        RunCommand::new(image)
    }

    /// Docker logs command
    pub fn logs(container_id: &str) -> LogsCommand {
        LogsCommand::new(container_id)
    }

    /// Docker inspect command
    pub fn inspect(container_id: &str) -> InspectCommand {
        InspectCommand::new(container_id)
    }

    /// Docker rm command
    pub fn rm(container_id: &str) -> RmCommand {
        RmCommand::new(container_id)
    }

    /// Docker stop command
    pub fn stop(container_id: &str) -> StopCommand {
        StopCommand::new(container_id)
    }
}
