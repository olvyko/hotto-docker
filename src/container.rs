use crate::{Image, InspectCommand, LogsCommand, RmCommand, RunCommand, StopCommand, WaitError};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

const ONE_SECOND: Duration = Duration::from_secs(1);
const ZERO: Duration = Duration::from_secs(0);

pub struct DockerContainer<I>
where
    I: Image,
{
    id: String,
    start_time: std::time::Instant,
    image: I,
}

impl<I> DockerContainer<I>
where
    I: Image,
{
    pub async fn new(image: I) -> Result<Self, WaitError> {
        let id = RunCommand::create_container(&image).await;
        let start_time = std::time::Instant::now();
        log::trace!("Registering starting of container {} at {:?}", id, start_time);
        let container = DockerContainer { id, start_time, image };
        wait_at_least_one_second_after_container_was_started(&container.id, &container.start_time).await;
        LogsCommand::wait_until_ready(&container.id, container.image().wait_for()).await?;
        Ok(container)
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn image(&self) -> &I {
        &self.image
    }

    pub async fn print_stdout(&self) {
        wait_at_least_one_second_after_container_was_started(&self.id, &self.start_time).await;
        LogsCommand::print_stdout(&self.id).await;
    }

    pub async fn print_stderr(&self) {
        wait_at_least_one_second_after_container_was_started(&self.id, &self.start_time).await;
        LogsCommand::print_stderr(&self.id).await;
    }

    async fn run_background_logs(&self, stdout: bool, stderr: bool) {
        wait_at_least_one_second_after_container_was_started(&self.id, &self.start_time).await;
        let id = self.id.clone();
        log::warn!("Starting new thread for background logs of container {}", self.id);
        std::thread::spawn(move || {
            let mut tokio_runtime = Runtime::new().expect("Unable to create tokio runtime");
            tokio_runtime.block_on(async {
                if stdout && stderr {
                    tokio::join!(LogsCommand::print_stdout(&id), LogsCommand::print_stderr(&id));
                } else if stdout {
                    LogsCommand::print_stdout(&id).await;
                } else if stderr {
                    LogsCommand::print_stderr(&id).await;
                }
            });
        });
    }

    pub async fn run_background_logs_all(&self) {
        self.run_background_logs(true, true).await;
    }

    pub async fn run_background_logs_stdout(&self) {
        self.run_background_logs(true, false).await;
    }

    pub async fn run_background_logs_stderr(&self) {
        self.run_background_logs(false, true).await;
    }

    /// Returns the mapped host port for an internal port of this docker container.
    ///
    /// This method does **not** magically expose the given port, it simply performs a mapping on
    /// the already exposed ports. If a docker image does not expose a port, this method will not
    /// be able to resolve it.
    pub async fn get_host_port(&self, internal_port: u16) -> Option<u16> {
        let resolved_port = InspectCommand::get_container_ports(&self.id)
            .await
            .map_to_host_port(internal_port);
        match resolved_port {
            Some(port) => log::debug!("Resolved port {} to {} for container {}", internal_port, port, self.id),
            None => log::warn!("Unable to resolve port {} for container {}", internal_port, self.id),
        }
        resolved_port
    }

    fn stop(&self) {
        log::debug!("Stopping docker container {}", self.id);
        StopCommand::stop_container(&self.id);
    }

    fn rm(&self) {
        log::debug!("Droping docker container {}", self.id);
        RmCommand::rm_container(&self.id);
    }
}

async fn wait_at_least_one_second_after_container_was_started(container_id: &str, start_time: &Instant) {
    let duration = Instant::now() - *start_time;
    log::trace!("Time since container {} was started: {:?}", container_id, duration);
    if duration < ONE_SECOND {
        tokio::time::delay_for(ONE_SECOND.checked_sub(duration).unwrap_or_else(|| ZERO)).await;
    }
}

/// The destructor implementation for a DockerContainer.
///
/// As soon as the container goes out of scope, the destructor will either only stop or delete the docker container.
/// This behaviour can be controlled through the `KEEP_CONTAINERS` environment variable. Setting it to `true` will only stop containers instead of removing them. Any other or no value will remove the container.
impl<I> Drop for DockerContainer<I>
where
    I: Image,
{
    fn drop(&mut self) {
        let keep_container = std::env::var("KEEP_CONTAINERS")
            .ok()
            .and_then(|var| var.parse().ok())
            .unwrap_or(false);
        match keep_container {
            true => self.stop(),
            false => self.rm(),
        }
    }
}
