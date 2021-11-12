use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::str::FromStr;

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientConf {
    listen_addr: String,
    port: u32,
    password: String,
    server_addr: String,
    server_port: u32,
}

impl ClientConf {
    pub fn to_listen_addr(&self) -> SocketAddr {
        format!("{}:{}", self.listen_addr, self.port)
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
