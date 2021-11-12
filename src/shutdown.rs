use tokio::sync::broadcast::Receiver;

#[derive(Debug)]
pub struct Shutdown {
    notify: Receiver<()>,
    shutdown: bool,
}

impl Shutdown {
    pub fn new(notify: Receiver<()>) -> Self {
        Self {
            notify,
            shutdown: false,
        }
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }

    pub async fn recv(&mut self) {
        if self.is_shutdown() {
            return;
        }

        let _ = self.notify.recv().await;
        self.shutdown = true;
    }
}
