use crate::conf::ClientConf;
use crate::shutdown::Shutdown;
use crate::{Error, Result};
use crate::MAX_CONNECTIONS;
use std::future::Future;
use std::result::Result::Ok;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Semaphore};
use tracing::{info, error};
use crate::cipher::Cipher;


#[derive(Debug)]
pub struct Client {
    conf: Arc<ClientConf>,
    listener: TcpListener,
    max_connections: Arc<Semaphore>,
    shutdown_notify: broadcast::Sender<()>,
    shutdown_complete_s: mpsc::Sender<()>,
    shutdown_complete_r: mpsc::Receiver<()>,
    cipher: Arc<Cipher>,
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
                stream,
                cipher: self.cipher.clone(),
                conf: self.conf.clone(),
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
    cipher: Arc<Cipher>,
    conf: Arc<ClientConf>,
}

impl Handler {
    pub async fn run(&mut self) -> Result<()> {
        // 链接服务端
        let addr = self.conf.to_server_listen_addr();
        let mut remote = TcpStream::connect(addr).await?;

        let (mut lr, mut lw) = self.stream.split();
        let (mut rr, mut rw) = remote.split();

        let client_to_server = async {
            let mut buff = [0; 1024];
            while let Ok(n) = lr.read(&mut buff).await {
                if n == 0 {
                    break;
                }
                let buff = &buff[..n];
                let mut encode_data = vec![];
                self.cipher.encode(buff, &mut encode_data);
                rw.write(&mut encode_data[..]).await?;
                rw.flush().await?;
            };
            Ok::<(), Error>(())
        };

        let server_to_client = async {
            let mut buff = [0; 1024];
            while let Ok(n) = rr.read(&mut buff).await {
                if n == 0 {
                    break;
                }
                let buff = &buff[..n];
                let mut decode_data = vec![];
                self.cipher.decode(buff, &mut decode_data);
                lw.write(&mut decode_data[..]).await?;
                lw.flush().await?;
            };

            Ok::<(), Error>(())
        };


        tokio::select! {
            _ = client_to_server => {}
            _ = server_to_client => {}
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
        cipher: Arc::new(Cipher::new(conf.password.clone())),
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
