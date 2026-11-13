//! Running package-manager processes without flashing console windows.

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

pub struct Captured {
    pub stdout: String,
    pub stderr: String,
}

pub fn cmd(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Runs to completion and returns all output. Errors only if the process can't start.
pub fn capture(c: &mut Command) -> Result<Captured, String> {
    let out = c
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not start {:?}: {e}", c.get_program()))?;
    Ok(Captured {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Runs and calls `on_line` for every line (split on \n and \r so progress bars come through).
/// Returns Err if the process exits with a non-zero code.
pub fn stream(c: &mut Command, on_line: &mut dyn FnMut(&str)) -> Result<(), String> {
    let mut child = c
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start {:?}: {e}", c.get_program()))?;

    let (tx, rx) = mpsc::channel::<String>();
    let pumps = [
        pump(child.stdout.take().expect("piped stdout"), tx.clone()),
        pump(child.stderr.take().expect("piped stderr"), tx),
    ];
    for line in rx {
        on_line(&line);
    }
    for p in pumps {
        let _ = p.join();
    }

    let status = child.wait().map_err(|e| e.to_string())?;
    match status.code() {
        Some(0) => Ok(()),
        Some(code) => Err(format!("exit code {code} (0x{:08X})", code as u32)),
        None => Err("process was terminated".into()),
    }
}

fn pump<R: Read + Send + 'static>(mut r: R, tx: mpsc::Sender<String>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut line = Vec::new();
        let mut buf = [0u8; 4096];
        let flush = |line: &mut Vec<u8>| {
            let s = String::from_utf8_lossy(line).trim().to_string();
            line.clear();
            // Skip blank lines and single-char spinner frames (- \ | /).
            if s.chars().count() > 1 {
                let _ = tx.send(s);
            }
        };
        loop {
            match r.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    for &b in &buf[..n] {
                        if b == b'\n' || b == b'\r' {
                            flush(&mut line);
                        } else {
                            line.push(b);
                        }
                    }
                }
            }
        }
        flush(&mut line);
    })
}
