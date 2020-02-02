use crate::{ContainerInfo, Image, WaitError};
use std::{
    collections::HashMap,
    process::Stdio,
    time::{Duration, SystemTime},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    stream::StreamExt,
};

pub struct RunCommand;

impl RunCommand {
    pub async fn create_container<I: Image>(image: &I) -> String {
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
        let child = command.spawn().expect("Failed to execute docker run command");
        let stdout = child.stdout.expect("failed to unwrap stdout docker run command");
        let reader = BufReader::new(stdout);
        let container_id = reader.lines().next().await.unwrap().unwrap();
        container_id
    }
}

pub struct LogsCommand;

impl LogsCommand {
    pub async fn wait_for_message_in_stdout(
        container_id: &str,
        message: &str,
        wait_duration: Duration,
    ) -> Result<(), WaitError> {
        let child = Command::new("docker")
            .arg("logs")
            .arg("-f")
            .arg(container_id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn docker logs command");
        let stdout = child.stdout.expect("failed to unwrap stdout docker logs command");
        let mut reader = BufReader::new(stdout).lines();
        let mut number_of_compared_lines = 0;
        let start_time = SystemTime::now();
        while let Some(line) = reader.next_line().await.unwrap() {
            number_of_compared_lines += 1;
            if line.contains(message) {
                log::info!("Found message after comparing {} lines", number_of_compared_lines);
                return Ok(());
            };
            if SystemTime::now().duration_since(start_time).unwrap() >= wait_duration {
                log::error!("Failed to find message in stream wait duration expired.");
                return Err(WaitError::WaitDurationExpired);
            };
        }
        log::error!(
            "Failed to find message in stream after comparing {} lines.",
            number_of_compared_lines
        );
        Err(WaitError::EndOfStream)
    }

    pub async fn wait_for_message_in_stderr(
        container_id: &str,
        message: &str,
        wait_duration: Duration,
    ) -> Result<(), WaitError> {
        let child = Command::new("docker")
            .arg("logs")
            .arg("-f")
            .arg(container_id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn docker logs command");
        let stderr = child.stderr.expect("failed to unwrap stderr docker logs command");
        let mut reader = BufReader::new(stderr).lines();
        let mut number_of_compared_lines = 0;
        let start_time = SystemTime::now();
        while let Some(line) = reader.next_line().await.unwrap() {
            number_of_compared_lines += 1;
            if line.contains(message) {
                log::info!("Found message after comparing {} lines", number_of_compared_lines);
                return Ok(());
            };
            if SystemTime::now().duration_since(start_time).unwrap() >= wait_duration {
                log::error!("Failed to find message in stream wait duration expired.");
                return Err(WaitError::WaitDurationExpired);
            };
        }
        log::error!(
            "Failed to find message in stream after comparing {} lines.",
            number_of_compared_lines
        );
        Err(WaitError::EndOfStream)
    }

    pub async fn print_stdout(container_id: &str) {
        let child = Command::new("docker")
            .arg("logs")
            .arg("-f")
            .arg(container_id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn docker logs command");
        let stdout = child.stdout.expect("failed to unwrap stdout docker logs command");
        let mut reader = BufReader::new(stdout).lines();
        let mut short_container_id = container_id.to_owned();
        short_container_id.truncate(6);
        while let Some(line) = reader.next_line().await.unwrap() {
            log::info!("stdout:{} > {}", short_container_id, line);
        }
    }

    pub async fn print_stderr(container_id: &str) {
        let child = Command::new("docker")
            .arg("logs")
            .arg("-f")
            .arg(container_id)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn docker logs command");
        let stderr = child.stderr.expect("failed to unwrap stderr docker logs command");
        let mut reader = BufReader::new(stderr).lines();
        let mut short_container_id = container_id.to_owned();
        short_container_id.truncate(6);
        while let Some(line) = reader.next_line().await.unwrap() {
            log::error!("stderr:{} > {}", short_container_id, line);
        }
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

pub struct InspectCommand;

impl InspectCommand {
    pub async fn get_container_info(container_id: &str) -> ContainerInfo {
        let child = Command::new("docker")
            .arg("inspect")
            .arg(container_id)
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn docker inspect command");
        let stdout = child.stdout.expect("failed to unwrap stdout docker inspect command");
        let mut reader = BufReader::new(stdout).lines();
        let mut buffer = String::new();
        while let Some(line) = reader.next_line().await.unwrap() {
            buffer.push_str(&line);
        }
        let mut infos = serde_json::from_str::<Vec<ContainerInfo>>(&buffer).unwrap();
        let info = infos.remove(0);
        log::trace!("Fetched container info: {:#?}", info);
        info
    }

    pub async fn get_container_ports(container_id: &str) -> Ports {
        InspectCommand::get_container_info(container_id).await.get_ports()
    }
}

pub struct RmCommand;

impl RmCommand {
    #[allow(unused_must_use)]
    pub async fn rm_container(container_id: &str) {
        Command::new("docker")
            .arg("rm")
            .arg("-f")
            .arg("-v") // Also remove volumes
            .arg(container_id)
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn docker rm command")
            .await;
    }
}

pub struct StopCommand;

impl StopCommand {
    #[allow(unused_must_use)]
    pub async fn stop_container(container_id: &str) {
        Command::new("docker")
            .arg("stop")
            .arg(container_id)
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn docker stop command")
            .await;
    }
}
