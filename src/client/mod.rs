use crate::client::connection::Connection;
use crate::conf::ClientConf;
use crate::shutdown::Shutdown;
use crate::{Result};
use crate::MAX_CONNECTIONS;
use std::future::Future;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Semaphore};
use tracing::{info, error};

pub mod connection;

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
            let socket = self.accept().await.unwrap();

            let mut handler = Handler {
                connection: Connection::new(socket),
                max_connections: self.max_connections.clone(),
                shutdown: Shutdown::new(self.shutdown_notify.subscribe()),
                _shutdown_complete_s: self.shutdown_complete_s.clone(),
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
    connection: Connection,
    max_connections: Arc<Semaphore>,
    shutdown: Shutdown,
    _shutdown_complete_s: mpsc::Sender<()>,
}

impl Handler {
    pub async fn run(&mut self) -> Result<()> {
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {}
            _ = self.shutdown.recv() => {}
        }
        Ok(())
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

    // 等待所有的task处理完任务再退出
    let _ = shutdown_complete_r.recv().await;
    info!("shutdown complete");
}
