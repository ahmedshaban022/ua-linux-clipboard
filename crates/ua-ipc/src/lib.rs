//! Shared IPC client for UA Clipboard thin clients (ADR-0002): the CLI,
//! the panel and the daemon's self-connections all speak the same
//! JSON-line protocol over the same socket. One implementation, one place
//! to change the framing.

use std::io::{BufRead, BufReader, Write};

use ua_core::ipc::{Request, Response};

/// $UA_CLIPBOARD_SOCKET, else $XDG_RUNTIME_DIR/ua-clipboard.sock.
pub fn socket_path() -> String {
    std::env::var("UA_CLIPBOARD_SOCKET").unwrap_or_else(|_| {
        let runtime = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into()));
        format!("{runtime}/ua-clipboard.sock")
    })
}

/// Send one request, read one response.
#[cfg(target_os = "linux")]
pub fn request(req: &Request) -> Result<Response, String> {
    use std::os::unix::net::UnixStream;

    let path = socket_path();
    let mut stream = UnixStream::connect(&path)
        .map_err(|e| format!("connect {path}: {e} — is ua-clipboard-daemon running?"))?;
    let json = serde_json::to_string(req).map_err(|e| e.to_string())?;
    writeln!(stream, "{json}").map_err(|e| e.to_string())?;
    stream.flush().ok();
    read_response(&mut BufReader::new(stream))
}

fn read_response(reader: &mut impl BufRead) -> Result<Response, String> {
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    serde_json::from_str(line.trim()).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
pub fn request(_req: &Request) -> Result<Response, String> {
    Err("IPC client requires Linux".to_string())
}
