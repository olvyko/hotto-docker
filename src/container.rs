use crate::{Image, InspectCommand, LogsCommand, RmCommand, RunCommand, StopCommand, WaitError};
use std::time::{Duration, Instant};
use tokio::runtime::{Builder, Runtime};

const ONE_SECOND: Duration = Duration::from_secs(1);
const ZERO: Duration = Duration::from_secs(0);

pub struct DockerContainer<I>
where
    I: Image,
{
    id: String,
    start_time: std::time::Instant,
    runtime: Runtime,
    image: I,
}

impl<I> DockerContainer<I>
where
    I: Image,
{
    pub fn new(image: I) -> Result<Self, WaitError> {
        // FIXME don't unwrap
        let mut runtime = Builder::new().enable_all().basic_scheduler().build().unwrap();
        let id = runtime.block_on(RunCommand::create_container(&image));
        let start_time = std::time::Instant::now();
        log::trace!("Registering starting of container {} at {:?}", id, start_time);
        let mut container = DockerContainer {
            id,
            start_time,
            runtime,
            image,
        };
        wait_at_least_one_second_after_container_was_started(&container.id, &container.start_time);
        container.runtime.block_on(LogsCommand::wait_until_ready(
            &container.id,
            container.image().wait_for(),
        ))?;
        Ok(container)
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn image(&self) -> &I {
        &self.image
    }

    pub fn print_stdout(&mut self) {
        wait_at_least_one_second_after_container_was_started(&self.id, &self.start_time);
        self.runtime.block_on(LogsCommand::print_stdout(&self.id));
    }

    pub fn print_stderr(&mut self) {
        wait_at_least_one_second_after_container_was_started(&self.id, &self.start_time);
        self.runtime.block_on(LogsCommand::print_stderr(&self.id));
    }

    fn run_background_logs(&self, stdout: bool, stderr: bool) {
        wait_at_least_one_second_after_container_was_started(&self.id, &self.start_time);
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

    pub fn run_background_logs_all(&self) {
        self.run_background_logs(true, true);
    }

    pub fn run_background_logs_stdout(&self) {
        self.run_background_logs(true, false);
    }

    pub fn run_background_logs_stderr(&self) {
        self.run_background_logs(false, true);
    }

    /// Returns the mapped host port for an internal port of this docker container.
    ///
    /// This method does **not** magically expose the given port, it simply performs a mapping on
    /// the already exposed ports. If a docker image does not expose a port, this method will not
    /// be able to resolve it.
    pub fn get_host_port(&mut self, internal_port: u16) -> Option<u16> {
        let resolved_port = self
            .runtime
            .block_on(InspectCommand::get_container_ports(&self.id))
            .map_to_host_port(internal_port);
        match resolved_port {
            Some(port) => log::debug!("Resolved port {} to {} for container {}", internal_port, port, self.id),
            None => log::warn!("Unable to resolve port {} for container {}", internal_port, self.id),
        }
        resolved_port
    }

    fn stop(&mut self) {
        log::debug!("Stopping docker container {}", self.id);
        self.runtime.block_on(StopCommand::stop_container(&self.id));
    }

    fn drop(&mut self) {
        log::debug!("Droping docker container {}", self.id);
        self.runtime.block_on(RmCommand::rm_container(&self.id));
    }
}

fn wait_at_least_one_second_after_container_was_started(container_id: &str, start_time: &Instant) {
    let duration = Instant::now() - *start_time;
    log::trace!("Time since container {} was started: {:?}", container_id, duration);
    if duration < ONE_SECOND {
        std::thread::sleep(ONE_SECOND.checked_sub(duration).unwrap_or_else(|| ZERO))
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
            false => self.drop(),
        }
    }
}
