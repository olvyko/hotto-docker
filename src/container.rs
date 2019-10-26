use crate::{Docker, Image, Logs};
use std::env::var;

pub struct Container<I>
where
    I: Image,
{
    id: String,
    image: I,
}

impl<I> Container<I>
where
    I: Image,
{
    pub fn new(id: String, image: I) -> Self {
        let container = Container { id, image };
        container.block_until_ready();
        container
    }

    /// Returns the id of this container.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Gives access to the log streams of this container.
    pub fn logs(&self) -> Logs {
        Docker::logs(&self.id)
    }

    /// Returns the mapped host port for an internal port of this docker container.
    ///
    /// This method does **not** magically expose the given port, it simply performs a mapping on
    /// the already exposed ports. If a docker image does not expose a port, this method will not
    /// be able to resolve it.
    pub fn get_host_port(&self, internal_port: u16) -> Option<u16> {
        let resolved_port = Docker::ports(&self.id).map_to_host_port(internal_port);

        match resolved_port {
            Some(port) => {
                log::debug!(
                    "Resolved port {} to {} for container {}",
                    internal_port,
                    port,
                    self.id
                );
            }
            None => {
                log::warn!(
                    "Unable to resolve port {} for container {}",
                    internal_port,
                    self.id
                );
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
        Docker::stop(&self.id)
    }

    fn rm(&self) {
        log::debug!("Deleting docker container {}", self.id);
        Docker::rm(&self.id)
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
            true => self.stop(),
            false => self.rm(),
        }
    }
}
