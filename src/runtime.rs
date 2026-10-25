use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::app::{Backend, ForwardSpec, ManagedProcess, ProcessState};

pub struct SystemBackend;

impl Backend for SystemBackend {
    fn port_free(&self, port: u16) -> bool {
        TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
    }

    // {impl}`requirements.forwarding.ssh`
    // {impl}`requirements.forwarding.ssh_config`
    fn launch(&mut self, spec: &ForwardSpec) -> Result<Box<dyn ManagedProcess>, String> {
        let mut child = ssh_command(spec)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start SSH: {error}"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or("Could not capture SSH diagnostics.")?;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                match line {
                    Ok(line) => {
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(format!("Could not read SSH diagnostics: {error}"));
                        break;
                    }
                }
            }
        });
        Ok(Box::new(SshProcess {
            child,
            local_port: spec.local_port,
            stderr: receiver,
            stderr_lines: Vec::new(),
            started: Instant::now(),
            running: false,
        }))
    }

    fn open_browser(&mut self, port: u16) -> Result<(), String> {
        let url = format!("http://localhost:{port}");
        #[cfg(target_os = "macos")]
        let mut command = Command::new("open");
        #[cfg(target_os = "linux")]
        let mut command = Command::new("xdg-open");
        #[cfg(target_os = "windows")]
        let mut command = {
            let mut command = Command::new("cmd");
            command.args(["/C", "start", ""]);
            command
        };
        let status = command
            .arg(url)
            .status()
            .map_err(|error| format!("Could not launch the default browser: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Browser opener exited with {status}"))
        }
    }
}

fn ssh_command(spec: &ForwardSpec) -> Command {
    let mut command = Command::new("ssh");
    command
        .args([
            "-N",
            "-T",
            "-o",
            "BatchMode=yes",
            "-o",
            "ExitOnForwardFailure=yes",
            "-o",
            "ConnectTimeout=10",
            "-L",
        ])
        .arg(format!(
            "127.0.0.1:{}:localhost:{}",
            spec.local_port, spec.remote_port
        ))
        .arg("--")
        .arg(&spec.host);
    command
}

struct SshProcess {
    child: Child,
    local_port: u16,
    stderr: Receiver<String>,
    stderr_lines: Vec<String>,
    started: Instant,
    running: bool,
}

impl SshProcess {
    fn drain_stderr(&mut self) -> Option<String> {
        let mut fault = None;
        while let Ok(line) = self.stderr.try_recv() {
            if is_failure_line(&line) {
                fault = Some(line.clone());
            }
            self.stderr_lines.push(line);
        }
        fault
    }

    fn diagnostics(&mut self, prefix: &str) -> String {
        if let Ok(line) = self.stderr.recv_timeout(Duration::from_millis(50)) {
            self.stderr_lines.push(line);
        }
        self.drain_stderr();
        if self.stderr_lines.is_empty() {
            prefix.to_owned()
        } else {
            format!("{prefix}\n{}", self.stderr_lines.join("\n"))
        }
    }
}

fn is_failure_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("failed") || lower.contains("refused") || lower.contains("error")
}

impl ManagedProcess for SshProcess {
    // {impl}`requirements.forwarding.startup_state`
    // {impl}`requirements.errors.visible`
    fn poll(&mut self) -> ProcessState {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                ProcessState::Exited(self.diagnostics(&format!("SSH exited with {status}.")))
            }
            Err(error) => ProcessState::Exited(format!("Could not check SSH process: {error}")),
            Ok(None) => {
                if let Some(error) = self.drain_stderr() {
                    return ProcessState::Fault(error);
                }
                if self.running {
                    return ProcessState::Running;
                }
                let address = SocketAddr::from((Ipv4Addr::LOCALHOST, self.local_port));
                if TcpStream::connect_timeout(&address, Duration::from_millis(20)).is_ok() {
                    self.running = true;
                    ProcessState::Running
                } else if self.started.elapsed() > Duration::from_secs(15) {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    ProcessState::Exited(
                        self.diagnostics("SSH did not open the local port in time."),
                    )
                } else {
                    ProcessState::Starting
                }
            }
        }
    }

    fn stop(&mut self) -> Result<(), String> {
        match self.child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => {
                self.child
                    .kill()
                    .map_err(|error| format!("Could not stop SSH: {error}"))?;
                self.child
                    .wait()
                    .map_err(|error| format!("Could not reap SSH: {error}"))?;
                Ok(())
            }
            Err(error) => Err(format!("Could not check SSH process: {error}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // {test}`requirements.forwarding.ssh`
    // {test}`requirements.forwarding.ssh_config`
    #[test]
    fn ssh_receives_alias_and_forward_as_arguments() {
        let command = ssh_command(&ForwardSpec {
            host: "chronos".into(),
            remote_port: 8030,
            local_port: 8031,
        });
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect();
        assert!(args.iter().any(|arg| arg == "ExitOnForwardFailure=yes"));
        assert!(
            args.iter()
                .any(|arg| arg == "127.0.0.1:8031:localhost:8030")
        );
        assert_eq!(args[args.len() - 2], "--");
        assert_eq!(args[args.len() - 1], "chronos");
    }

    // {test}`requirements.errors.visible`
    #[test]
    fn recognizes_live_ssh_failure_diagnostics() {
        assert!(is_failure_line(
            "channel 3: open failed: connect failed: Connection refused"
        ));
        assert!(!is_failure_line("Warning: Permanently added host key"));
    }
}
