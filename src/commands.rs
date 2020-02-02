use crate::{Container, ContainerInfo, Image, WaitError};
use std::{
    cell::RefCell,
    collections::HashMap,
    process::Stdio,
    rc::Rc,
    time::{Duration, SystemTime},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    runtime::Runtime,
    stream::StreamExt,
};

pub struct RunCommand {
    tokio_runtime: Rc<RefCell<Runtime>>,
}

impl RunCommand {
    pub fn new(tokio_runtime: Rc<RefCell<Runtime>>) -> Self {
        Self { tokio_runtime }
    }

    async fn docker_run<I: Image>(image: I) -> String {
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

    pub async fn create_container<I: Image>(image: I) -> Result<Container<I>, WaitError> {
        let container_id = RunCommand::docker_run(image.clone()).await;
        Container::new(container_id, image.clone(), None).await
    }

    pub fn create_container_blocking<I: Image>(&self, image: I) -> Result<Container<I>, WaitError> {
        self.tokio_runtime.borrow_mut().block_on(async {
            let container_id = RunCommand::docker_run(image.clone()).await;
            Container::new(container_id, image.clone(), Some(self.tokio_runtime.clone())).await
        })
    }
}

pub struct LogsCommand {
    tokio_runtime: Rc<RefCell<Runtime>>,
}

impl LogsCommand {
    pub fn new(tokio_runtime: Rc<RefCell<Runtime>>) -> Self {
        Self { tokio_runtime }
    }

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

    pub fn wait_for_message_in_stdout_blocking(
        &self,
        container_id: &str,
        message: &str,
        wait_duration: Duration,
    ) -> Result<(), WaitError> {
        self.tokio_runtime
            .borrow_mut()
            .block_on(LogsCommand::wait_for_message_in_stdout(
                container_id,
                message,
                wait_duration,
            ))
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

    pub fn wait_for_message_in_stderr_blocking(
        &self,
        container_id: &str,
        message: &str,
        wait_duration: Duration,
    ) -> Result<(), WaitError> {
        self.tokio_runtime
            .borrow_mut()
            .block_on(LogsCommand::wait_for_message_in_stderr(
                container_id,
                message,
                wait_duration,
            ))
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

pub struct InspectCommand {
    tokio_runtime: Rc<RefCell<Runtime>>,
}

impl InspectCommand {
    pub fn new(tokio_runtime: Rc<RefCell<Runtime>>) -> Self {
        Self { tokio_runtime }
    }

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

    pub fn get_container_info_blocking(&self, container_id: &str) -> ContainerInfo {
        self.tokio_runtime
            .borrow_mut()
            .block_on(InspectCommand::get_container_info(container_id))
    }

    pub async fn get_container_ports(container_id: &str) -> Ports {
        InspectCommand::get_container_info(container_id).await.get_ports()
    }

    pub fn get_container_ports_blocking(&self, container_id: &str) -> Ports {
        self.get_container_info_blocking(container_id).get_ports()
    }
}

pub struct RmCommand {
    tokio_runtime: Rc<RefCell<Runtime>>,
}

impl RmCommand {
    pub fn new(tokio_runtime: Rc<RefCell<Runtime>>) -> Self {
        Self { tokio_runtime }
    }

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

    pub fn rm_container_blocking(&self, container_id: &str) {
        self.tokio_runtime
            .borrow_mut()
            .block_on(RmCommand::rm_container(container_id));
    }
}

pub struct StopCommand {
    tokio_runtime: Rc<RefCell<Runtime>>,
}

impl StopCommand {
    pub fn new(tokio_runtime: Rc<RefCell<Runtime>>) -> Self {
        Self { tokio_runtime }
    }

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

    pub fn stop_container_blocking(&self, container_id: &str) {
        self.tokio_runtime
            .borrow_mut()
            .block_on(StopCommand::stop_container(container_id));
    }
}
