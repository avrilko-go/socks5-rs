use std::future::Future;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use bytes::Buf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Semaphore};
use crate::cipher::Cipher;
use crate::MAX_CONNECTIONS;
use crate::{Result, Error};
use crate::shutdown::Shutdown;
use tracing::{error, info};

pub struct Server {
    cipher: Arc<Cipher>,
    listener: TcpListener,
    max_connections: Arc<Semaphore>,
    shutdown_notify: broadcast::Sender<()>,
    shutdown_complete_s: mpsc::Sender<()>,
    shutdown_complete_r: mpsc::Receiver<()>,
}

impl Server {
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
            };

            tokio::spawn(async move {
                if let Err(err) = handler.run().await {
                    error!(cause = ?err, "connection error");
                }
            });
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
}

impl Drop for Handler {
    fn drop(&mut self) {
        info!("connection quit");
        self.max_connections.add_permits(1);
    }
}

impl Handler {
    pub async fn run(&mut self) -> Result<()> {
        let mut header = [0; 3];
        self.stream.read(&mut header).await?;
        let mut header_decode = vec![];
        self.cipher.decode(&header, &mut header_decode);

        if header_decode[0] != 5 {
            return Err("only support socks5".into());
        }
        let mut encode_buffer = vec![];
        self.cipher.encode(&[5, 0], &mut encode_buffer);
        self.stream.write(&mut encode_buffer).await?;
        self.stream.flush().await?;


        let mut header = [0; 256];
        let n = self.stream.read(&mut header).await?;
        let header = &header[..n];

        let mut header_decode = vec![];
        self.cipher.decode(header, &mut header_decode);
        match header_decode[3] {
            3 => {
                let mut port = &header_decode[n - 2..];
                let port = port.get_u16();
                let domain = std::str::from_utf8(&header_decode[5..n - 2]).unwrap();
                let addr = format!("{}:{}", domain, port).to_socket_addrs()?.next().unwrap();
                let mut remote = TcpStream::connect(addr).await?;

                let mut encode_buffer = vec![];
                self.cipher.encode(&[0x05, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], &mut encode_buffer);
                self.stream.write(&mut encode_buffer).await?;
                self.stream.flush().await?;

                let (mut lr, mut lw) = self.stream.split();
                let (mut rr, mut rw) = remote.split();


                let client_to_server = async {
                    let mut buff = [0; 1024];
                    while let Ok(n) = lr.read(&mut buff).await {
                        if n == 0 {
                            break;
                        }
                        let buff = &buff[..n];
                        let mut decode_data = vec![];
                        self.cipher.decode(buff, &mut decode_data);
                        rw.write(&mut decode_data[..]).await?;
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
                        let mut encode_data = vec![];
                        self.cipher.encode(buff, &mut encode_data);
                        lw.write(&mut encode_data[..]).await?;
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
            _ => {
                return Err("only support domain".into());
            }
        }
    }
}


pub async fn run(listener: TcpListener, shutdown: impl Future, password: String) {
    let (shutdown_notify, _) = broadcast::channel(1);
    let (shutdown_complete_s, shutdown_complete_r) = mpsc::channel(1);

    let server = Server {
        cipher: Arc::new(Cipher::new(password)),
        listener,
        max_connections: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
        shutdown_notify,
        shutdown_complete_s,
        shutdown_complete_r,
    };

    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                error!(cause = %err, "failed to accept");
            }
        }
        _ = shutdown =>{
            info!("shutdown......");
        }
    }

    let Server {
        shutdown_notify,
        shutdown_complete_s,
        mut shutdown_complete_r,
        ..
    } = server;
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
