use super::*;
use std::sync::atomic::Ordering;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub(super) async fn server(
    body_stalls: Option<bool>,
) -> (String, oneshot::Receiver<()>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (started, receiver) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (mut connection, _) = listener.accept().await.unwrap();
        let mut buffer = [0u8; 1024];
        connection.read(&mut buffer).await.unwrap();
        match body_stalls {
            Some(true) => {
                connection
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\na")
                    .await
                    .unwrap();
            }
            None => {
                connection
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc")
                    .await
                    .unwrap();
                started.send(()).unwrap();
                return;
            }
            Some(false) => {}
        }
        started.send(()).unwrap();
        std::future::pending::<()>().await;
        drop(connection);
    });
    (format!("http://{address}/checkpoint"), receiver, task)
}

async fn check_stall(body_stalls: bool, cancel_request: bool) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("model.safetensors");
    let flag = Arc::new(AtomicBool::new(false));
    let (url, started, stalled_server) = server(Some(body_stalls)).await;
    let request = tokio::spawn(download_file(
        DownloadOptions::new(url, &path)
            .with_minimum_bytes(3)
            .with_cancel_flag(flag.clone()),
        flag.clone(),
        if cancel_request {
            Duration::from_secs(30)
        } else {
            Duration::from_millis(200)
        },
    ));
    tokio::time::timeout(Duration::from_secs(2), started)
        .await
        .unwrap()
        .unwrap();
    if body_stalls {
        tokio::time::timeout(Duration::from_secs(2), async {
            while !root.path().join("model.safetensors.tmp").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    if cancel_request {
        flag.store(true, Ordering::Relaxed);
    }
    let result = tokio::time::timeout(Duration::from_secs(2), request).await;
    stalled_server.abort();
    let error = result
        .expect("a stalled managed download must terminate promptly")
        .unwrap()
        .unwrap_err();
    if cancel_request {
        assert_eq!(error, "cancelled");
    } else {
        assert!(error.contains("download timed out"), "{error}");
    }
    assert!(!path.exists());
    assert!(!root.path().join("model.safetensors.tmp").exists());
    assert!(!root
        .path()
        .join("model.safetensors.download-complete.json")
        .exists());
    let (url, _started, retry_server) = server(None).await;
    download_file(
        DownloadOptions::new(url, &path).with_minimum_bytes(3),
        Arc::new(AtomicBool::new(false)),
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    retry_server.await.unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"abc");
}

#[tokio::test]
async fn stalled_headers_cancel_promptly_and_retry() {
    check_stall(false, true).await;
}

#[tokio::test]
async fn stalled_body_cancels_promptly_cleans_partial_and_retries() {
    check_stall(true, true).await;
}

#[tokio::test]
async fn stalled_headers_time_out_and_retry() {
    check_stall(false, false).await;
}

#[tokio::test]
async fn stalled_body_times_out_cleans_partial_and_retries() {
    check_stall(true, false).await;
}
