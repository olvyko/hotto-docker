use crate::{Container, WaitError, WaitForMessage};
use async_trait::async_trait;
use std::collections::HashMap;

/// Represents a docker image.
#[async_trait]
pub trait Image: Sized + Clone + Default {
    fn descriptor(&self) -> String;
    async fn wait_until_ready(&self, container: &Container<Self>);
    fn env_vars(&self) -> HashMap<String, String>;
    fn args(&self) -> Vec<String>;
    fn mounts(&self) -> Vec<HashMap<String, String>>;
    fn network(&self) -> Option<String>;
    fn with_args(self, args: Vec<String>) -> Self;
}

#[derive(Debug, PartialEq, Clone)]
pub enum WaitFor {
    Nothing,
    LogMessage { message: String, stream: Stream },
}

#[derive(Debug, PartialEq, Clone)]
pub enum Stream {
    StdOut,
    StdErr,
}

impl WaitFor {
    pub fn message_on_stdout<S: Into<String>>(message: S) -> WaitFor {
        WaitFor::LogMessage {
            message: message.into(),
            stream: Stream::StdOut,
        }
    }

    pub fn message_on_stderr<S: Into<String>>(message: S) -> WaitFor {
        WaitFor::LogMessage {
            message: message.into(),
            stream: Stream::StdErr,
        }
    }

    async fn wait<I: Image>(&self, container: &Container<I>) -> Result<(), WaitError> {
        match self {
            WaitFor::Nothing => Ok(()),
            WaitFor::LogMessage { message, stream } => match stream {
                Stream::StdOut => WaitForMessage::wait_for_message(container.stdout_stream(), message).await,
                Stream::StdErr => WaitForMessage::wait_for_message(container.stderr_stream(), message).await,
            },
        }
    }
}

#[derive(Clone)]
pub struct GenericImage {
    descriptor: String,
    env_vars: HashMap<String, String>,
    args: Vec<String>,
    mounts: Vec<HashMap<String, String>>,
    network: Option<String>,
    wait_for: WaitFor,
}

impl Default for GenericImage {
    fn default() -> Self {
        Self {
            descriptor: "".to_owned(),
            env_vars: HashMap::new(),
            args: vec![],
            mounts: vec![],
            network: None,
            wait_for: WaitFor::Nothing,
        }
    }
}

impl GenericImage {
    pub fn new<S: Into<String>>(descriptor: S) -> GenericImage {
        Self {
            descriptor: descriptor.into(),
            env_vars: HashMap::new(),
            args: vec![],
            mounts: vec![],
            network: None,
            wait_for: WaitFor::Nothing,
        }
    }

    pub fn with_env_var<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.env_vars.insert(key.into(), value.into());
        self
    }

    pub fn with_mount(mut self, mount: HashMap<String, String>) -> Self {
        self.mounts.push(mount);
        self
    }

    pub fn with_network(mut self, network: String) -> Self {
        self.network = Some(network);
        self
    }

    pub fn with_wait_for(mut self, wait_for: WaitFor) -> Self {
        self.wait_for = wait_for;
        self
    }
}

#[async_trait]
impl Image for GenericImage {
    fn descriptor(&self) -> String {
        self.descriptor.to_owned()
    }

    async fn wait_until_ready(&self, container: &Container<Self>) {
        self.wait_for.wait(container).await.unwrap();
    }

    fn env_vars(&self) -> HashMap<String, String> {
        self.env_vars.clone()
    }

    fn args(&self) -> Vec<String> {
        self.args.clone()
    }

    fn mounts(&self) -> Vec<HashMap<String, String>> {
        self.mounts.clone()
    }

    fn network(&self) -> Option<String> {
        self.network.clone()
    }

    fn with_args(self, args: Vec<String>) -> Self {
        Self { args, ..self }
    }
}
