use crate::{Container, ContainerInfo, Image};
use async_stream::stream;
use std::cell::RefCell;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    stream::{Stream, StreamExt},
};

/// Implementation of the Docker client API using the docker cli.
pub struct Docker;

impl Docker {
    pub fn run<I: Image>(image: I) -> RunCommand<I> {
        RunCommand::new(image)
    }

    pub fn logs(container_id: &str) -> LogsCommand {
        LogsCommand::new(container_id)
    }

    pub fn inspect(container_id: &str) -> InspectCommand {
        InspectCommand::new(container_id)
    }

    pub fn rm(container_id: &str) -> RmCommand {
        RmCommand::new(container_id)
    }

    pub fn stop(container_id: &str) -> StopCommand {
        StopCommand::new(container_id)
    }
}

pub struct RunCommand<I> {
    image: I,
    command: RefCell<Command>,
}

impl<I: Image> RunCommand<I> {
    pub fn new(image: I) -> Self {
        let run = RunCommand::<I> {
            image: image.clone(),
            command: RefCell::new(Command::new("docker")),
        };
        run.command.borrow_mut().arg("run");
        // Environment variables
        for (key, value) in image.env_vars() {
            run.command.borrow_mut().arg("-e").arg(format!("{}={}", key, value));
        }
        // Mounts
        for value in image.mounts() {
            run.command.borrow_mut().arg("--mount").arg(
                value
                    .iter()
                    .map(|(key, value)| format!("{}={}", key, value))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        // Network
        if let Some(network) = image.network() {
            run.command.borrow_mut().arg("--network").arg(network);
        }
        run.command
            .borrow_mut()
            .arg("-d") // Always run detached
            .arg("-P") // Always expose all ports
            .arg(image.descriptor())
            .args(image.args())
            .stdout(Stdio::piped());
        run
    }

    pub async fn create_container(&self) -> Container<I> {
        log::debug!("Executing command: {:?}", self.command.borrow());
        let child = self
            .command
            .borrow_mut()
            .spawn()
            .expect("Failed to execute docker run command");
        let stdout = child.stdout.expect("failed to unwrap stdout docker run command");
        let reader = BufReader::new(stdout);
        let container_id = reader.lines().next().await.unwrap().unwrap();
        Container::new(container_id, self.image.clone())
    }
}

pub struct LogsCommand {
    command: RefCell<Command>,
}

impl LogsCommand {
    pub fn new(container_id: &str) -> Self {
        let logs = LogsCommand {
            command: RefCell::new(Command::new("docker")),
        };
        logs.command
            .borrow_mut()
            .arg("logs")
            .arg("-f")
            .arg(container_id)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        logs
    }

    pub fn stdout_stream(&self) -> impl Stream<Item = String> {
        let child = self
            .command
            .borrow_mut()
            .spawn()
            .expect("failed to spawn docker logs command");
        let stdout = child.stdout.expect("failed to unwrap stdout docker logs command");
        let mut reader = BufReader::new(stdout).lines();
        stream! {
            while let Some(line) = reader.next_line().await.unwrap() {
                yield line;
            }
        }
    }

    pub fn stderr_stream(&self) -> impl Stream<Item = String> {
        let child = self
            .command
            .borrow_mut()
            .spawn()
            .expect("failed to spawn docker logs command");
        let stderr = child.stderr.expect("failed to unwrap stdout docker logs command");
        let mut reader = BufReader::new(stderr).lines();
        stream! {
            while let Some(line) = reader.next_line().await.unwrap() {
                yield line;
            }
        }
    }

    pub async fn print_stdout(&self) {
        let child = self
            .command
            .borrow_mut()
            .spawn()
            .expect("failed to spawn docker logs command");
        let stdout = child.stdout.expect("failed to unwrap stdout docker logs command");
        let mut reader = BufReader::new(stdout).lines();
        while let Some(line) = reader.next_line().await.unwrap() {
            println!("{}", line);
        }
    }

    pub async fn print_stderr(&self) {
        let child = self
            .command
            .borrow_mut()
            .spawn()
            .expect("failed to spawn docker logs command");
        let stderr = child.stderr.expect("failed to unwrap stderr docker logs command");
        let mut reader = BufReader::new(stderr).lines();
        while let Some(line) = reader.next_line().await.unwrap() {
            println!("{}", line);
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
    command: RefCell<Command>,
}

impl InspectCommand {
    pub fn new(container_id: &str) -> Self {
        let inspect = InspectCommand {
            command: RefCell::new(Command::new("docker")),
        };
        inspect
            .command
            .borrow_mut()
            .arg("inspect")
            .arg(container_id)
            .stdout(Stdio::piped());
        inspect
    }

    pub async fn get_container_info(&self) -> ContainerInfo {
        let child = self
            .command
            .borrow_mut()
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

    pub async fn get_container_ports(&self) -> Ports {
        self.get_container_info().await.get_ports()
    }
}

pub struct RmCommand {
    command: RefCell<Command>,
}

impl RmCommand {
    pub fn new(container_id: &str) -> Self {
        let rm = RmCommand {
            command: RefCell::new(Command::new("docker")),
        };
        rm.command
            .borrow_mut()
            .arg("rm")
            .arg("-f")
            .arg("-v") // Also remove volumes
            .arg(container_id)
            .stdout(Stdio::piped());
        rm
    }

    pub async fn rm_container(&self) {
        let child = self
            .command
            .borrow_mut()
            .spawn()
            .expect("failed to spawn docker rm command")
            .await;
    }
}

pub struct StopCommand {
    command: RefCell<Command>,
}

impl StopCommand {
    pub fn new(container_id: &str) -> Self {
        let stop = StopCommand {
            command: RefCell::new(Command::new("docker")),
        };
        stop.command
            .borrow_mut()
            .arg("stop")
            .arg(container_id)
            .stdout(Stdio::piped());
        stop
    }

    pub async fn stop_container(&self) {
        let child = self
            .command
            .borrow_mut()
            .spawn()
            .expect("failed to spawn docker stop command")
            .await;
    }
}
