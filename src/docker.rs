use crate::{Container, ContainerInfo, Image};
use std::{
    collections::HashMap,
    io::Read,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

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

        Container::new(container_id, image)
    }

    pub fn logs(id: &str) -> Logs {
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
