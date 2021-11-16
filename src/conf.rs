use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::str::FromStr;

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientConf {
    pub listen_addr: String,
    pub port: u32,
    pub password: String,
    pub server_addr: String,
    pub server_port: u32,
}

impl ClientConf {
    pub fn to_listen_addr(&self) -> SocketAddr {
        format!("{}:{}", self.listen_addr, self.port)
            .parse()
            .unwrap()
    }

    pub fn to_server_listen_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server_addr, self.server_port)
            .parse()
            .unwrap()
    }
}

impl FromStr for ClientConf {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}


#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConf {
    pub listen_addr: String,
    pub port: u32,
}

impl ServerConf {
    pub fn to_listen_addr(&self) -> SocketAddr {
        format!("{}:{}", self.listen_addr, self.port)
            .parse()
            .unwrap()
    }
}

impl FromStr for ServerConf {
    type Err = serde_json::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(s)
    }
}
