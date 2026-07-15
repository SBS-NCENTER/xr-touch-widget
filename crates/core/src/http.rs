//! Minimal HTTP GET for button actions (D16). One call = one request:
//! Ok from ureq (2xx, redirects auto-followed) = success; an HTTP error
//! status, transport error, timeout or bad URL = Err(reason). The response
//! body is ignored — the caller only needs fired-or-failed.
//! Lives in core (no Tauri) so it unit-tests against a plain TcpListener,
//! the same philosophy as mock-xr for the OSC path.

use std::time::Duration;

/// Fire a GET at `url`. Blocking — callers that must not stall (the engine
/// loop) run it on a spawned thread. `timeout` covers the whole call
/// (connect + response); the engine passes its HTTP_TIMEOUT constant, tests
/// pass short values.
pub fn get(url: &str, timeout: Duration) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new().timeout(timeout).build();
    match agent.get(url).call() {
        Ok(_) => Ok(()),
        // 4xx/5xx arrive as Error::Status — surface the code (the operator
        // sees it in the log; the palette only needs "failed").
        Err(ureq::Error::Status(code, _)) => Err(format!("HTTP {code}")),
        // Transport-level: refused, DNS, timeout, malformed URL, ...
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    const T: Duration = Duration::from_millis(500);

    /// One-shot local HTTP server answering the first request with `response`.
    fn serve_once(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(response.as_bytes());
            }
        });
        // Query string included: the Pixotope publish URL shape must pass
        // through ureq untouched.
        format!("http://{addr}/gateway/25.2.4/publish?Type=Call&ParamNumber=0")
    }

    #[test]
    fn ok_on_200() {
        let url = serve_once("HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n[]");
        assert_eq!(get(&url, T), Ok(()));
    }

    #[test]
    fn err_on_500_with_status_in_reason() {
        let url =
            serve_once("HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\n\r\n");
        let err = get(&url, T).unwrap_err();
        assert!(err.contains("500"), "want status in reason, got: {err}");
    }

    #[test]
    fn err_on_connection_refused() {
        // Bind to grab a free port, then drop the listener so nothing answers.
        let addr = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        assert!(get(&format!("http://{addr}/"), T).is_err());
    }

    #[test]
    fn err_on_timeout() {
        // Accepts and reads, then never responds — get() must give up at
        // `timeout` (200ms here so the test stays fast), not hang.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                std::thread::sleep(Duration::from_secs(2)); // > timeout
            }
        });
        assert!(get(&format!("http://{addr}/"), Duration::from_millis(200)).is_err());
    }

    #[test]
    fn err_on_bad_url() {
        assert!(get("not a url", T).is_err());
    }
}
