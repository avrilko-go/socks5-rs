pub type Result<T> = std::result::Result<T, Error>;
pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub const MAX_CONNECTIONS: usize = 100;

pub mod conf;

pub mod client;
pub mod shutdown;
