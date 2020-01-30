use crate::{Docker, Image};
use std::{
    collections::HashMap,
    env::var,
    sync::RwLock,
    thread::sleep,
    time::{Duration, Instant},
};
use tokio::stream::Stream;

const ONE_SECOND: Duration = Duration::from_secs(1);
const ZERO: Duration = Duration::from_secs(0);

pub struct Container<I>
where
    I: Image,
{
    id: String,
    startup_timestamps: RwLock<HashMap<String, Instant>>,
    image: I,
}

impl<I> Container<I>
where
    I: Image,
{
    pub fn new(id: String, image: I) -> Self {
        let container = Container {
            id,
            startup_timestamps: RwLock::default(),
            image,
        };
        container.register_container_started();
        container.block_until_ready();
        container
    }

    /// Returns the id of this container.
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn stdout_stream(&self) -> impl Stream<Item = String> {
        Docker::logs(&self.id).stdout_stream()
    }

    pub fn stderr_stream(&self) -> impl Stream<Item = String> {
        Docker::logs(&self.id).stderr_stream()
    }

    pub async fn print_stdout(&self) {
        self.wait_at_least_one_second_after_container_was_started();
        Docker::logs(&self.id).print_stdout().await;
    }

    pub async fn print_stderr(&self) {
        self.wait_at_least_one_second_after_container_was_started();
        Docker::logs(&self.id).print_stderr().await;
    }

    /// Returns the mapped host port for an internal port of this docker container.
    ///
    /// This method does **not** magically expose the given port, it simply performs a mapping on
    /// the already exposed ports. If a docker image does not expose a port, this method will not
    /// be able to resolve it.
    pub async fn get_host_port(&self, internal_port: u16) -> Option<u16> {
        let resolved_port = Docker::inspect(&self.id)
            .get_container_ports()
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

    fn block_until_ready(&self) {
        log::debug!("Waiting for container {} to be ready", self.id);
        self.image.wait_until_ready(self);
        log::debug!("Container {} is now ready!", self.id);
    }

    fn stop(&self) {
        log::debug!("Stopping docker container {}", self.id);
        block_on(Docker::stop(&self.id).stop_container());
    }

    fn rm(&self) {
        log::debug!("Deleting docker container {}", self.id);
        block_on(Docker::rm(&self.id).rm_container());
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

use futures_executor::block_on;

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
            true => self.stop(),
            false => self.rm(),
        }
    }
}
