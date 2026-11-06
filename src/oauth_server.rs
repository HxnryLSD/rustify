use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

/// Captures the Spotify authorization code from the OAuth 2.0 callback.
///
/// Spins up a tiny local HTTP server on the configured redirect port. When
/// Spotify redirects the user's browser back after authorization, the server
/// reads the `code` query parameter and returns an HTML page telling the user
/// they can close the tab.
pub struct OAuthServer {
    port: u16,
    captured_code: Arc<Mutex<Option<String>>>,
    captured_error: Arc<Mutex<Option<String>>>,
}

impl OAuthServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            captured_code: Arc::new(Mutex::new(None)),
            captured_error: Arc::new(Mutex::new(None)),
        }
    }

    /// Start the server, open the browser to Spotify's authorization URL, and
    /// block until the callback arrives. Returns the authorization code.
    pub fn wait_for_callback(
        &self,
        auth_url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.port))?;

        let code = self.captured_code.clone();
        let error = self.captured_error.clone();

        // Accept loop in a background thread so we can poll.
        let _accept_thread = thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        let code = code.clone();
                        let error = error.clone();
                        thread::spawn(move || {
                            let mut buf = [0u8; 4096];
                            let _n = stream.read(&mut buf).unwrap_or(0);
                            let req = String::from_utf8_lossy(&buf);

                            let code_val = req
                                .lines()
                                .next()
                                .and_then(|l| l.split_whitespace().nth(1))
                                .and_then(|path| {
                                    let (_, query) = path.split_once('?')?;
                                    let mut params: Vec<(&str, &str)> = Vec::new();
                                    for pair in query.split('&') {
                                        if let Some((k, v)) = pair.split_once('=') {
                                            params.push((k, v));
                                        }
                                    }
                                    params.iter().find(|(k, _)| *k == "code").map(|(_, v)| *v)
                                })
                                .map(|s| s.to_string());

                            let error_val = req
                                .lines()
                                .next()
                                .and_then(|l| l.split_whitespace().nth(1))
                                .and_then(|path| {
                                    let (_, query) = path.split_once('?')?;
                                    let mut params: Vec<(&str, &str)> = Vec::new();
                                    for pair in query.split('&') {
                                        if let Some((k, v)) = pair.split_once('=') {
                                            params.push((k, v));
                                        }
                                    }
                                    params.iter().find(|(k, _)| *k == "error").map(|(_, v)| *v)
                                });

                            let body: String = if let Some(c) = &code_val {
                                *code.lock().unwrap() = Some(c.clone());
                                "<html><body><h1>Authorized!</h1><p>You can close this tab and return to Rustify.</p></body></html>".to_string()
                            } else if let Some(err) = error_val {
                                *error.lock().unwrap() = Some(err.to_string());
                                format!(
                                    "<html><body><h1>Authorization failed</h1><p>{err}</p></body></html>"
                                )
                            } else {
                                "<html><body><h1>Rustify OAuth callback</h1><p>No code or error received.</p></body></html>".to_string()
                            };

                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                body.len(),
                                body
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.shutdown(std::net::Shutdown::Both);
                        });
                    }
                    Err(_) => break,
                }
            }
        });

        // Open the system browser.
        if let Err(e) = webbrowser::open(auth_url) {
            return Err(format!("Failed to open browser: {e}").into());
        }

        // Poll for the captured code/error.
        loop {
            thread::sleep(std::time::Duration::from_millis(100));
            if let Some(c) = self.captured_code.lock().unwrap().take() {
                return Ok(c);
            }
            if let Some(e) = self.captured_error.lock().unwrap().take() {
                return Err(format!("OAuth error: {e}").into());
            }
        }
    }
}
