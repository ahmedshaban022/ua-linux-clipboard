//! Tiny IPC client used by `--capture-stdin` and other self-connections.

use std::io::{BufRead, BufReader, Write};

use ua_core::ipc::{Request, Response};

pub fn request(req: &Request) -> Result<Response, String> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::net::UnixStream;
        let path = crate::socket_path();
        let mut stream = UnixStream::connect(&path).map_err(|e| format!("connect {path}: {e}"))?;
        let json = serde_json::to_string(req).map_err(|e| e.to_string())?;
        writeln!(stream, "{json}").map_err(|e| e.to_string())?;
        stream.flush().ok();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        serde_json::from_str(line.trim()).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = req;
        Err("IPC client requires Linux".to_string())
    }
}
