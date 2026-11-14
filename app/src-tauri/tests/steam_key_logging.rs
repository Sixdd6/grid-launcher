//! Spike 4 of `docs/stores-phase4-plan.md`, kept as a regression test: the
//! Steam Web API key can only travel in a request's query string, so no
//! log line from reqwest, hyper, hyper-util, h2 or rustls may ever carry a
//! request URL. This runs key-carrying requests (200, 302 with and without
//! redirects, 401, a refused connection, a timeout) against a local server
//! under a `TRACE` filter for every target, with the app's subscriber shape
//! (`tracing_subscriber` fmt layer; `init` also bridges the `log` crate,
//! which reqwest and rustls use), and greps every captured line for the
//! fixture key. GRID's "debug prints" switch sets the filter to `debug`,
//! which `trace` covers.
//!
//! It also pins down why `stores/steam/api.rs` must call
//! `reqwest::Error::without_url()`: a `reqwest::Error`'s `Display` and
//! `Debug` carry the full URL, query and key included.
//!
//! An integration test, so it has its own process and its own global
//! subscriber.

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

const KEY: &str = "FAKE-STEAM-KEY-not-real";

/// Every formatted log line, in memory.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Captured;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// A tiny HTTP/1.1 server: `/ok` 200 JSON, `/moved` 302 to `/ok` on the
/// same host, `/denied` 401 with Steam's HTML, `/slow` answers after 5 s.
async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = request.split_whitespace().nth(1).unwrap_or("/").to_string();
            let response = if path.starts_with("/ok") {
                let body = r#"{"response":{"game_count":0,"games":[]}}"#;
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                    body.len()
                )
            } else if path.starts_with("/moved") {
                "HTTP/1.1 302 Found\r\nlocation: /ok\r\ncontent-length: 0\r\n\r\n".to_string()
            } else if path.starts_with("/denied") {
                let body = "<html><head><title>Unauthorized</title></head><body><h1>Unauthorized</h1>Access is denied. Retrying will not help. Please verify your <pre>key=</pre> parameter.</body></html>";
                format!(
                    "HTTP/1.1 401 Unauthorized\r\ncontent-type: text/html\r\ncontent-length: {}\r\n\r\n{body}",
                    body.len()
                )
            } else {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n".to_string()
            };
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        });
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_log_line_carries_a_key_from_a_request_url() {
    let captured = Captured::default();
    tracing_subscriber::registry()
        .with(EnvFilter::new("trace"))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(captured.clone()),
        )
        .try_init()
        .expect("the only subscriber in this process");

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(serve(listener));
    // A port nothing listens on: bind, note it, close.
    let refused = {
        let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", dead.local_addr().unwrap())
    };

    // As the Steam client is built: no redirects. And a default client,
    // which follows them, to see whether following logs the target.
    let no_redirects = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();
    let follows = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();

    let query = [("key", KEY), ("steamid", "76561198000000001")];
    let mut errors = Vec::new();
    for (client, url) in [
        (&no_redirects, format!("{base}/ok")),
        (&no_redirects, format!("{base}/moved")),
        (&follows, format!("{base}/moved")),
        (&no_redirects, format!("{base}/denied")),
        (&no_redirects, format!("{base}/slow")),
        (&no_redirects, format!("{refused}/ok")),
    ] {
        match client.get(&url).query(&query).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let _ = response.bytes().await;
                tracing::info!(target: "spike", "status {status}");
            }
            Err(error) => errors.push(error),
        }
    }
    assert_eq!(errors.len(), 2, "the timeout and the refused connection");

    // A reqwest error names the URL, key and all, until `without_url`.
    for error in errors {
        let shown = format!("{error} {error:?}");
        assert!(
            shown.contains(KEY),
            "reqwest changed its error text: {shown}"
        );
        let error = error.without_url();
        let shown = format!("{error} {error:?}");
        assert!(!shown.contains(KEY), "without_url kept the key: {shown}");
    }

    // Let background connection tasks log their last lines.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let text = String::from_utf8_lossy(&captured.0.lock().unwrap()).to_string();
    if std::env::var_os("GRID_SPIKE_DUMP").is_some() {
        eprintln!("{text}");
    }
    assert!(
        text.contains("status 200") && text.contains("status 302") && text.contains("status 401"),
        "the capture works:\n{text}"
    );
    assert!(
        text.lines().count() > 3,
        "the libraries logged at trace:\n{text}"
    );
    let leaks: Vec<&str> = text.lines().filter(|line| line.contains(KEY)).collect();
    assert!(
        leaks.is_empty(),
        "the key reached a log line:\n{}",
        leaks.join("\n")
    );
    assert!(
        !text.contains("steamid=") && !text.contains("/ok?"),
        "a request URL reached a log line:\n{text}"
    );
}
