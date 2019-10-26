use crate::{Container, ContainerInfo, Image};
use std::{
    collections::HashMap,
    io::Read,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::RwLock,
    thread::sleep,
    time::{Duration, Instant},
};

use lazy_static::*;

lazy_static! {
    static ref CONTAINER_STARTUP_TIMESTAMPS: RwLock<HashMap<String, Instant>> = RwLock::default();
}

const ONE_SECOND: Duration = Duration::from_secs(1);
const ZERO: Duration = Duration::from_secs(0);

fn register_container_started(id: String) {
    let mut lock_guard = match CONTAINER_STARTUP_TIMESTAMPS.write() {
        Ok(lock_guard) => lock_guard,
        // We only need the mutex
        // Data cannot be in-consistent even if a thread panics while holding the lock
        Err(e) => e.into_inner(),
    };
    let start_timestamp = Instant::now();
    log::trace!(
        "Registering starting of container {} at {:?}",
        id,
        start_timestamp
    );
    lock_guard.insert(id, start_timestamp);
}

fn time_since_container_was_started(id: &str) -> Option<Duration> {
    let lock_guard = match CONTAINER_STARTUP_TIMESTAMPS.read() {
        Ok(lock_guard) => lock_guard,
        // We only need the mutex
        // Data cannot be in-consistent even if a thread panics while holding the lock
        Err(e) => e.into_inner(),
    };
    let result = lock_guard.get(id).map(|i| Instant::now() - *i);
    log::trace!("Time since container {} was started: {:?}", id, result);
    result
}

fn wait_at_least_one_second_after_container_was_started(id: &str) {
    if let Some(duration) = time_since_container_was_started(id) {
        if duration < ONE_SECOND {
            sleep(ONE_SECOND.checked_sub(duration).unwrap_or_else(|| ZERO))
        }
    }
}

/// Implementation of the Docker client API using the docker cli.
pub struct Docker;

impl Docker {
    pub fn run<I: Image>(image: I) -> Container<I> {
        let mut command = Command::new("docker");
        command.arg("run");

        // Environment variables
        for (key, value) in image.env_vars() {
            command.arg("-e").arg(format!("{}={}", key, value));
        }
        // Mounts
        for value in image.mounts() {
            command.arg("--mount").arg(
                value
                    .iter()
                    .map(|(key, value)| format!("{}={}", key, value))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        // Network
        if let Some(network) = image.network() {
            command.arg("--network").arg(network);
        }

        command
            .arg("-d") // Always run detached
            .arg("-P") // Always expose all ports
            .arg(image.descriptor())
            .args(image.args())
            .stdout(Stdio::piped());

        log::debug!("Executing command: {:?}", command);
        let child = command.spawn().expect("Failed to execute docker command");

        let stdout = child.stdout.unwrap();
        let reader = BufReader::new(stdout);
        let container_id = reader.lines().next().unwrap().unwrap();

        register_container_started(container_id.clone());
        Container::new(container_id, image)
    }

    pub fn logs(id: &str) -> Logs {
        wait_at_least_one_second_after_container_was_started(id);

        let child = Command::new("docker")
            .arg("logs")
            .arg("-f")
            .arg(id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to execute docker command");

        Logs {
            stdout: Box::new(child.stdout.unwrap()),
            stderr: Box::new(child.stderr.unwrap()),
        }
    }

    pub fn ports(id: &str) -> Ports {
        let child = Command::new("docker")
            .arg("inspect")
            .arg(id)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to execute docker command");

        let stdout = child.stdout.unwrap();
        let mut infos: Vec<ContainerInfo> = serde_json::from_reader(stdout).unwrap();
        let info = infos.remove(0);

        log::trace!("Fetched container info: {:#?}", info);
        info.get_ports()
    }

    pub fn rm(id: &str) {
        Command::new("docker")
            .arg("rm")
            .arg("-f")
            .arg("-v") // Also remove volumes
            .arg(id)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to execute docker command");
    }

    pub fn stop(id: &str) {
        Command::new("docker")
            .arg("stop")
            .arg(id)
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to execute docker command");
    }
}

/// The exposed ports of a running container.
#[derive(Debug, PartialEq, Default)]
pub struct Ports {
    mapping: HashMap<u16, u16>,
}

impl Ports {
    /// Registers the mapping of an exposed port.
    pub fn add_mapping(&mut self, internal: u16, host: u16) -> &mut Self {
        log::debug!("Registering port mapping: {} -> {}", internal, host);
        self.mapping.insert(internal, host);
        self
    }

    /// Returns the host port for the given internal port.
    pub fn map_to_host_port(&self, internal_port: u16) -> Option<u16> {
        self.mapping.get(&internal_port).cloned()
    }
}

/// Log streams of running container (stdout & stderr).
pub struct Logs {
    pub stdout: Box<dyn Read>,
    pub stderr: Box<dyn Read>,
}
