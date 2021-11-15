use crate::conf::ClientConf;
use crate::shutdown::Shutdown;
use crate::{Result};
use crate::MAX_CONNECTIONS;
use std::future::Future;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Semaphore};
use tracing::{info, error};
use bytes::Buf;


#[derive(Debug)]
pub struct Client {
    conf: Arc<ClientConf>,
    listener: TcpListener,
    max_connections: Arc<Semaphore>,
    shutdown_notify: broadcast::Sender<()>,
    shutdown_complete_s: mpsc::Sender<()>,
    shutdown_complete_r: mpsc::Receiver<()>,
}

impl Client {
    pub async fn run(&self) -> Result<()> {
        info!("begin accept info");

        loop {
            self.max_connections.acquire().await.unwrap().forget();
            let stream = self.accept().await.unwrap();

            let mut handler = Handler {
                max_connections: self.max_connections.clone(),
                shutdown: Shutdown::new(self.shutdown_notify.subscribe()),
                _shutdown_complete_s: self.shutdown_complete_s.clone(),
                stream
            };

            tokio::spawn(async move {
                if let Err(err) = handler.run().await {
                    error!(cause = ?err, "connection error");
                }
            });
        }
    }


    pub async fn accept(&self) -> Result<TcpStream> {
        let mut wait = 1;
        loop {
            match self.listener.accept().await {
                Ok((tcp_stream, _)) => {
                    return Ok(tcp_stream);
                }
                Err(err) => {
                    if wait > 60 {
                        return Err(err.into());
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(wait)).await;
                    wait *= 2;
                }
            }
        }
    }
}

#[derive(Debug)]
struct Handler {
    stream: TcpStream,
    max_connections: Arc<Semaphore>,
    shutdown: Shutdown,
    _shutdown_complete_s: mpsc::Sender<()>,
}

impl Handler {
    pub async fn run(&mut self) -> Result<()> {
        let mut header = [0;3];
        self.stream.read(&mut header).await?;
        if header[0] != 5 {
            return Err("only support socks5".into())
        }
        self.stream.write_u8(5).await?;
        self.stream.write_u8(0).await?;
        self.stream.flush().await?;
        let mut header = [0;256];
        let n = self.stream.read(&mut header).await?;
        let header = &header[..n];
        match header[3] {
            3 => {
                let mut port = &header[n-2..];
                let port = port.get_u16();
                let domain = std::str::from_utf8(&header[5..n-2]).unwrap();
                let addr = format!("{}:{}",domain,port).to_socket_addrs()?.next().unwrap();
                let mut remote = TcpStream::connect(addr).await?;
                let mut ack = [0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
                self.stream.write(&mut ack).await?;
                self.stream.flush().await?;

                let (mut lr,mut lw) = self.stream.split();
                let (mut rr,mut rw) = remote.split();
                let client_to_server = async {
                    tokio::io::copy(&mut lr,&mut rw).await
                };

                let server_to_client = async {
                    tokio::io::copy(&mut rr,&mut lw).await
                };

                tokio::select! {
                    _ = client_to_server => {}
                    _ = server_to_client => {}
                    _ = self.shutdown.recv() => {
                        info!("receive quit signal");
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_secs(10)) => {
                        info!("on connection timeout");
                    }
                };
                Ok(())
            }
            _ => {
                return Err("only support domain".into());
            }
        }
    }
}

impl Drop for Handler {
    fn drop(&mut self) {
        info!("connection quit");
        self.max_connections.add_permits(1);
    }
}

pub async fn run(listener: TcpListener, shutdown: impl Future, conf: ClientConf) {
    let (shutdown_notify, _) = broadcast::channel(1);
    let (shutdown_complete_s, shutdown_complete_r) = mpsc::channel(1);

    let client = Client {
        conf: Arc::new(conf),
        listener,
        max_connections: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
        shutdown_notify,
        shutdown_complete_s,
        shutdown_complete_r,
    };

    tokio::select! {
        res = client.run() => {
            if let Err(err) = res {
                error!(cause = %err, "failed to accept");
            }
        }
        _ = shutdown =>{
            info!("shutdown......");
        }
    }

    let Client {
        shutdown_notify,
        shutdown_complete_s,
        mut shutdown_complete_r,
        ..
    } = client;
    drop(shutdown_notify);
    drop(shutdown_complete_s);

    tokio::select! {
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(10))=> {
            info!("timeout wait conn quit");
        }

        _ = shutdown_complete_r.recv() =>  {
            info!("normal quit");
        }
    }
    info!("shutdown complete");
}
