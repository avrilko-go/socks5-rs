pub type Result<T> = std::result::Result<T, Error>;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub const MAX_CONNECTIONS: usize = 1024;

pub mod conf;

pub mod client;
pub mod shutdown;
pub mod cipher;
pub mod server;
