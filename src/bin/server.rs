use async_recursion::async_recursion;
use std::collections::HashMap;
use std::result::Result::Ok;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    let h = Arc::new(Mutex::new(HashMap::new()));
    read_name("/home/avrilko/web/fol.2345.net/models", h.clone()).await;

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    dbg!("{}", h);
}

#[async_recursion]
async fn read_name(path: &str, h: Arc<Mutex<HashMap<String, i32>>>) {
    let mut dirs = tokio::fs::read_dir(path).await.unwrap();

    while let Ok(Some(dir)) = dirs.next_entry().await {
        let path = dir.path();
        let meta = tokio::fs::metadata(&path).await.unwrap();
        if meta.is_dir() {
            let copy = h.clone();
            tokio::spawn(async move { read_name(path.to_str().unwrap(), copy).await });
        } else {
            let mut ddd = h.lock().unwrap();
            if ddd.contains_key(path.to_str().unwrap()) {
                ddd.insert(path.to_str().unwrap().to_string(), 2);
            } else {
                ddd.insert(path.to_str().unwrap().to_string(), 1);
            }
        }
    }
}
