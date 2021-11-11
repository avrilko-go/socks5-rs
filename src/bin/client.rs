use structopt::StructOpt;
use socks::Result;
use serde::{Serialize, Deserialize};
use socks::conf::ClientConf;


#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().try_init()?;
    let cli = Cli::from_args();
    let conf_path = cli.conf_path.unwrap_or_else(|| {
        dirs::home_dir().unwrap().join("config.json").to_str().unwrap().to_string()
    });
    let conf = tokio::fs::read_to_string(conf_path).await?;
    let a: ClientConf = conf.parse()?;
    dbg!(a.to_listen_addr());

    Ok(())
}

#[derive(Debug, StructOpt)]
#[structopt(name = "socks5 client", version = env ! ("CARGO_PKG_VERSION"), author = env ! ("CARGO_PKG_AUTHORS"), about = env ! ("CARGO_PKG_DESCRIPTION"))]
struct Cli {
    #[structopt(name = "conf path", short = "c", long = "conf")]
    conf_path: Option<String>,
}