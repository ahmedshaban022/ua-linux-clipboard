//! JSON-line IPC over a Unix socket. One request per line, one response
//! per line. The GNOME extension, ua-cli and ua-gtk all speak this; the
//! DBus mirror (spec §2) rides on the same `Request`/`Response` types.

use std::io::{BufRead, BufReader, Write};

use ua_core::ipc::{Request, Response};

use crate::DaemonState;

#[cfg(target_os = "linux")]
pub fn serve(
    path: &str,
    state: std::sync::Arc<std::sync::Mutex<DaemonState>>,
) -> std::io::Result<()> {
    use std::os::unix::net::UnixListener;

    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = std::sync::Arc::clone(&state);
                std::thread::spawn(move || handle_conn(stream, state));
            }
            Err(e) => eprintln!("[daemon] accept error: {e}"),
        }
    }
    Ok(())
}

/// Development fallback for non-Linux hosts: speak the same protocol on
/// stdin/stdout so the full request surface stays exercisable.
#[cfg(not(target_os = "linux"))]
pub fn serve(
    _path: &str,
    state: std::sync::Arc<std::sync::Mutex<DaemonState>>,
) -> std::io::Result<()> {
    eprintln!("[daemon] unix sockets unavailable; serving IPC on stdin (dev mode)");
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let mut out = std::io::stdout();
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let resp = serde_json::from_str::<Request>(trimmed)
                    .map_err(|e| e.to_string())
                    .and_then(|req| {
                        let mut s = state.lock().unwrap();
                        Ok(crate::handle_request(&mut s, req))
                    })
                    .unwrap_or_else(|e| Response::Error(e));
                let json = serde_json::to_string(&resp).unwrap_or_default();
                let _ = writeln!(out, "{json}");
                let _ = out.flush();
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn handle_conn(
    stream: std::os::unix::net::UnixStream,
    state: std::sync::Arc<std::sync::Mutex<DaemonState>>,
) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut out = stream;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let resp = match serde_json::from_str::<Request>(trimmed) {
                    Ok(req) => {
                        let mut s = state.lock().unwrap();
                        crate::handle_request(&mut s, req)
                    }
                    Err(e) => Response::Error(format!("bad request: {e}")),
                };
                let json = serde_json::to_string(&resp).unwrap_or_default();
                if writeln!(out, "{json}").is_err() {
                    break;
                }
                let _ = out.flush();
            }
        }
    }
}
