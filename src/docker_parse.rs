use crate::Ports as DockerPorts;
use std::collections::HashMap;

#[derive(serde::Deserialize, Debug)]
struct NetworkSettings {
    #[serde(rename = "Ports")]
    ports: Ports,
}

#[derive(serde::Deserialize, Debug)]
struct PortMapping {
    #[serde(rename = "HostIp")]
    ip: String,
    #[serde(rename = "HostPort")]
    port: String,
}

#[derive(serde::Deserialize, Debug)]
pub struct ContainerInfo {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "NetworkSettings")]
    network_settings: NetworkSettings,
}

impl ContainerInfo {
    pub fn get_ports(self) -> DockerPorts {
        self.network_settings.ports.into_ports()
    }
}

#[derive(serde::Deserialize, Debug)]
struct Ports(HashMap<String, Option<Vec<PortMapping>>>);

impl Ports {
    pub fn into_ports(self) -> DockerPorts {
        let mut ports = DockerPorts::default();

        for (internal, external) in self.0 {
            let external = match external.and_then(|mut m| m.pop()).map(|m| m.port) {
                Some(port) => port,
                None => {
                    log::debug!("Port {} is not mapped to host machine, skipping.", internal);
                    continue;
                }
            };

            let port = internal.split('/').next().unwrap();

            let internal = Self::parse_port(port);
            let external = Self::parse_port(&external);

            ports.add_mapping(internal, external);
        }
        ports
    }

    fn parse_port(port: &str) -> u16 {
        port.parse()
            .unwrap_or_else(|e| panic!("Failed to parse {} as u16 because {}", port, e))
    }
}
