use std::collections::HashMap;

/// Represents a docker image.
pub trait Image: Sized + Clone + Default {
    fn descriptor(&self) -> String;
    fn wait_for(&self) -> WaitFor {
        WaitFor::Nothing
    }
    fn env_vars(&self) -> HashMap<String, String>;
    fn args(&self) -> Vec<String>;
    fn mounts(&self) -> Vec<HashMap<String, String>>;
    fn network(&self) -> Option<String>;
    fn with_args(self, args: Vec<String>) -> Self;
}

#[derive(Debug, PartialEq, Clone)]
pub enum WaitFor {
    Nothing,
    LogMessage {
        message: String,
        stream_type: StreamType,
        wait_duration: u64,
    },
}

#[derive(Debug, PartialEq, Clone)]
pub enum StreamType {
    StdOut,
    StdErr,
}

impl WaitFor {
    pub fn message_on_stdout<S: Into<String>>(message: S, wait_duration: u64) -> WaitFor {
        WaitFor::LogMessage {
            message: message.into(),
            stream_type: StreamType::StdOut,
            wait_duration,
        }
    }

    pub fn message_on_stderr<S: Into<String>>(message: S, wait_duration: u64) -> WaitFor {
        WaitFor::LogMessage {
            message: message.into(),
            stream_type: StreamType::StdErr,
            wait_duration,
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

impl Image for GenericImage {
    fn descriptor(&self) -> String {
        self.descriptor.to_owned()
    }

    fn wait_for(&self) -> WaitFor {
        self.wait_for.clone()
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
