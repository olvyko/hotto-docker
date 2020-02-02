use crate::{Image, InspectCommand, LogsCommand, RmCommand, StopCommand, StreamType, WaitError, WaitFor};
use std::{
    cell::RefCell,
    collections::HashMap,
    env::var,
    rc::Rc,
    sync::RwLock,
    thread::sleep,
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;

const ONE_SECOND: Duration = Duration::from_secs(1);
const ZERO: Duration = Duration::from_secs(0);

pub struct Container<I>
where
    I: Image,
{
    id: String,
    startup_timestamps: RwLock<HashMap<String, Instant>>,
    tokio_runtime: Option<Rc<RefCell<Runtime>>>,
    image: I,
}

impl<I> Container<I>
where
    I: Image,
{
    pub async fn new(id: String, image: I) -> Result<Self, WaitError> {
        let container = Container {
            id,
            startup_timestamps: RwLock::default(),
            image,
            tokio_runtime: None,
        };
        container.register_container_started();
        container.block_until_ready().await?;
        Ok(container)
    }

    pub fn with_tokio_runtime(mut self, tokio_runtime: Rc<RefCell<Runtime>>) -> Self {
        self.tokio_runtime = Some(tokio_runtime);
        self
    }

    pub(crate) fn is_tokio_runtime_was_set(&self) -> bool {
        self.tokio_runtime.is_some()
    }

    /// Returns the id of this container.
    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn print_stdout(&self) {
        self.wait_at_least_one_second_after_container_was_started();
        LogsCommand::print_stdout(&self.id).await;
    }

    pub async fn print_stderr(&self) {
        self.wait_at_least_one_second_after_container_was_started();
        LogsCommand::print_stderr(&self.id).await;
    }

    pub fn run_background_logs(&self, stdout: bool, stderr: bool) {
        if !stdout && !stderr {
            log::warn!("NOT Starting new thread for background logs");
            return;
        };
        self.wait_at_least_one_second_after_container_was_started();
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
            Some(port) => {
                log::debug!("Resolved port {} to {} for container {}", internal_port, port, self.id);
            }
            None => {
                log::warn!("Unable to resolve port {} for container {}", internal_port, self.id);
            }
        }
        resolved_port
    }

    /// Blocking version of get_host_port func
    pub fn get_host_port_blocking(&self, internal_port: u16) -> Option<u16> {
        let inspect_command = InspectCommand::new(
            self.tokio_runtime
                .as_ref()
                .expect("dockerust > unable to use get_host_port_blocking func, tokio runtime hasn't initialized")
                .clone(),
        );
        let resolved_port = inspect_command
            .get_container_ports_blocking(&self.id)
            .map_to_host_port(internal_port);

        match resolved_port {
            Some(port) => {
                log::debug!("Resolved port {} to {} for container {}", internal_port, port, self.id);
            }
            None => {
                log::warn!("Unable to resolve port {} for container {}", internal_port, self.id);
            }
        }
        resolved_port
    }

    /// Returns a reference to the [`Image`] of this container.
    ///
    /// Access to this is useful if the [`arguments`] of the [`Image`] change how to connect to the
    /// Access to this is useful to retrieve [`Image`] specific information such as authentication details or other relevant information which have been passed as [`arguments`]
    ///
    /// [`Image`]: trait.Image.html
    /// [`arguments`]: trait.Image.html#associatedtype.Args
    pub fn image(&self) -> &I {
        &self.image
    }

    async fn block_until_ready(&self) -> Result<(), WaitError> {
        log::debug!("Waiting for container {} to be ready", self.id);
        match self.image.wait_for() {
            WaitFor::LogMessage {
                message,
                stream_type,
                wait_duration,
            } => match stream_type {
                StreamType::StdOut => {
                    LogsCommand::wait_for_message_in_stdout(&self.id, &message, wait_duration).await?
                }
                StreamType::StdErr => {
                    LogsCommand::wait_for_message_in_stderr(&self.id, &message, wait_duration).await?
                }
            },
            WaitFor::Nothing => {}
        }
        log::debug!("Container {} is now ready!", self.id);
        Ok(())
    }

    async fn stop(&self) {
        log::debug!("Stopping docker container {}", self.id);
        StopCommand::stop_container(&self.id).await;
    }

    fn stop_blocking(&mut self) {
        log::debug!("Stopping docker container {}", self.id);
        self.tokio_runtime
            .as_ref()
            .expect("dockerust > unable to use stop_blocking func, tokio runtime hasn't initialized")
            .borrow_mut()
            .block_on(StopCommand::stop_container(&self.id));
    }

    async fn rm(&self) {
        log::debug!("Deleting docker container {}", self.id);
        RmCommand::rm_container(&self.id).await;
    }

    fn rm_blocking(&mut self) {
        log::debug!("Deleting docker container {}", self.id);
        self.tokio_runtime
            .as_ref()
            .expect("dockerust > unable to use rm_blocking func, tokio runtime hasn't initialized")
            .borrow_mut()
            .block_on(RmCommand::rm_container(&self.id));
    }

    fn register_container_started(&self) {
        let mut lock_guard = match self.startup_timestamps.write() {
            Ok(lock_guard) => lock_guard,
            // We only need the mutex
            // Data cannot be in-consistent even if a thread panics while holding the lock
            Err(e) => e.into_inner(),
        };
        let start_timestamp = Instant::now();
        log::trace!("Registering starting of container {} at {:?}", self.id, start_timestamp);
        lock_guard.insert(self.id.clone(), start_timestamp);
    }

    fn time_since_container_was_started(&self) -> Option<Duration> {
        let lock_guard = match self.startup_timestamps.read() {
            Ok(lock_guard) => lock_guard,
            // We only need the mutex
            // Data cannot be in-consistent even if a thread panics while holding the lock
            Err(e) => e.into_inner(),
        };
        let result = lock_guard.get(&self.id).map(|i| Instant::now() - *i);
        log::trace!("Time since container {} was started: {:?}", self.id, result);
        result
    }

    fn wait_at_least_one_second_after_container_was_started(&self) {
        if let Some(duration) = self.time_since_container_was_started() {
            if duration < ONE_SECOND {
                sleep(ONE_SECOND.checked_sub(duration).unwrap_or_else(|| ZERO))
            }
        }
    }
}

/// The destructor implementation for a Container.
///
/// As soon as the container goes out of scope, the destructor will either only stop or delete the docker container.
/// This behaviour can be controlled through the `KEEP_CONTAINERS` environment variable. Setting it to `true` will only stop containers instead of removing them. Any other or no value will remove the container.
impl<I> Drop for Container<I>
where
    I: Image,
{
    fn drop(&mut self) {
        let keep_container = var("KEEP_CONTAINERS")
            .ok()
            .and_then(|var| var.parse().ok())
            .unwrap_or(false);

        match keep_container {
            true => self.stop_blocking(),
            false => self.rm_blocking(),
        }
    }
}
