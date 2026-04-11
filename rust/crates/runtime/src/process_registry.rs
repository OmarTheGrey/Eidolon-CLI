//! Background process registry — tracks long-running processes spawned by the
//! bash tool so the agent can check status, read output, and kill them later.
//!
//! When the model runs `cargo build --release` or `npm test` in background
//! mode, the process is registered here. The agent can continue working and
//! poll for completion without blocking the conversation.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Status of a tracked background process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Running,
    Completed { exit_code: Option<i32> },
    Failed { error: String },
    Killed,
}

impl std::fmt::Display for ProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Completed { exit_code: Some(code) } => write!(f, "completed (exit {code})"),
            Self::Completed { exit_code: None } => write!(f, "completed"),
            Self::Failed { error } => write!(f, "failed: {error}"),
            Self::Killed => write!(f, "killed"),
        }
    }
}

/// Metadata for a registered background process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessEntry {
    pub id: String,
    pub command: String,
    pub status: ProcessStatus,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub output_path: PathBuf,
    /// OS process ID (for display; not used for management after spawn).
    pub pid: Option<u32>,
}

/// Internal state for a live process (not serializable — holds the `Child`).
struct LiveProcess {
    child: Child,
    entry: ProcessEntry,
}

/// Thread-safe registry of background processes.
#[derive(Clone)]
pub struct ProcessRegistry {
    inner: Arc<Mutex<RegistryInner>>,
}

struct RegistryInner {
    processes: BTreeMap<String, LiveProcess>,
    /// Completed/failed processes that are no longer live.
    finished: BTreeMap<String, ProcessEntry>,
    /// Directory where stdout/stderr output files are stored.
    output_dir: PathBuf,
    /// Counter for generating unique IDs.
    next_id: u64,
}

#[allow(clippy::cast_possible_truncation)] // epoch millis fits in u64
fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl ProcessRegistry {
    /// Create a new registry that stores output files in `output_dir`.
    pub fn new(output_dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(output_dir)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(RegistryInner {
                processes: BTreeMap::new(),
                finished: BTreeMap::new(),
                output_dir: output_dir.to_path_buf(),
                next_id: 1,
            })),
        })
    }

    /// Register a spawned child process. Returns the assigned process ID.
    pub fn register(&self, command: &str, child: Child) -> String {
        let mut inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = format!("bg-{}", inner.next_id);
        inner.next_id += 1;

        let output_path = inner.output_dir.join(format!("{id}.log"));
        let pid = child.id();

        let entry = ProcessEntry {
            id: id.clone(),
            command: truncate_command(command),
            status: ProcessStatus::Running,
            started_at: now_epoch_ms(),
            finished_at: None,
            output_path,
            pid: Some(pid),
        };

        inner.processes.insert(
            id.clone(),
            LiveProcess {
                child,
                entry,
            },
        );

        id
    }

    /// Poll all running processes and update their status.
    /// Returns the IDs of any processes that just completed.
    pub fn poll(&self) -> Vec<String> {
        let mut inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut newly_done = Vec::new();

        let ids: Vec<String> = inner.processes.keys().cloned().collect();
        for id in ids {
            if let Some(live) = inner.processes.get_mut(&id) {
                match live.child.try_wait() {
                    Ok(Some(status)) => {
                        live.entry.status = exit_status_to_process_status(status);
                        live.entry.finished_at = Some(now_epoch_ms());
                        newly_done.push(id.clone());
                    }
                    Ok(None) => {} // still running
                    Err(e) => {
                        live.entry.status = ProcessStatus::Failed {
                            error: e.to_string(),
                        };
                        live.entry.finished_at = Some(now_epoch_ms());
                        newly_done.push(id.clone());
                    }
                }
            }
        }

        // Move completed processes to the finished map.
        for id in &newly_done {
            if let Some(live) = inner.processes.remove(id) {
                inner.finished.insert(id.clone(), live.entry);
            }
        }

        newly_done
    }

    /// List all processes (running + finished).
    pub fn list(&self) -> Vec<ProcessEntry> {
        self.poll(); // refresh status first
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut entries: Vec<ProcessEntry> = inner
            .processes
            .values()
            .map(|lp| lp.entry.clone())
            .chain(inner.finished.values().cloned())
            .collect();
        entries.sort_by_key(|e| e.started_at);
        entries
    }

    /// Get the status of a specific process.
    pub fn status(&self, id: &str) -> Option<ProcessEntry> {
        self.poll();
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        inner
            .processes
            .get(id)
            .map(|lp| lp.entry.clone())
            .or_else(|| inner.finished.get(id).cloned())
    }

    /// Read the output log for a process. Returns the file contents if the
    /// log file exists, or a status message if the process is still running.
    pub fn read_output(&self, id: &str) -> Result<String, String> {
        let entry = self.status(id).ok_or_else(|| format!("unknown process: {id}"))?;
        match fs::read_to_string(&entry.output_path) {
            Ok(content) => Ok(content),
            Err(_) if entry.status == ProcessStatus::Running => {
                Ok("(process is still running, output not yet available)".to_string())
            }
            Err(e) => Err(format!("failed to read output for {id}: {e}")),
        }
    }

    /// Kill a running process.
    pub fn kill(&self, id: &str) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(live) = inner.processes.get_mut(id) {
            live.child
                .kill()
                .map_err(|e| format!("failed to kill process {id}: {e}"))?;
            live.entry.status = ProcessStatus::Killed;
            live.entry.finished_at = Some(now_epoch_ms());
            let entry = live.entry.clone();
            inner.processes.remove(id);
            inner.finished.insert(id.to_string(), entry);
            Ok(())
        } else if inner.finished.contains_key(id) {
            Err(format!("process {id} already finished"))
        } else {
            Err(format!("unknown process: {id}"))
        }
    }

    /// Returns the output directory path.
    #[must_use]
    pub fn output_dir(&self) -> PathBuf {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.output_dir.clone()
    }

    /// Number of currently running processes.
    pub fn running_count(&self) -> usize {
        let inner = self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        inner.processes.len()
    }
}

fn exit_status_to_process_status(status: ExitStatus) -> ProcessStatus {
    ProcessStatus::Completed {
        exit_code: status.code(),
    }
}

fn truncate_command(cmd: &str) -> String {
    if cmd.len() <= 200 {
        cmd.to_string()
    } else {
        format!("{}...", &cmd[..197])
    }
}

impl std::fmt::Debug for ProcessRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessRegistry")
            .field("running", &self.running_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn test_output_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eidolon-process-registry-test-{}",
            now_epoch_ms()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn register_and_poll_tracks_process_lifecycle() {
        let dir = test_output_dir();
        let registry = ProcessRegistry::new(&dir).unwrap();

        // Spawn a short-lived process.
        let child = Command::new("echo")
            .arg("hello")
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("echo should spawn");

        let id = registry.register("echo hello", child);
        assert!(id.starts_with("bg-"));
        // Process may already be done by the time we check.

        // Give it a moment to finish, then poll.
        std::thread::sleep(std::time::Duration::from_millis(100));
        registry.poll();

        let entry = registry.status(&id).expect("should exist");
        assert!(
            matches!(entry.status, ProcessStatus::Completed { .. }),
            "echo should have completed: {:?}",
            entry.status
        );
    }

    #[test]
    fn list_returns_all_processes() {
        let dir = test_output_dir();
        let registry = ProcessRegistry::new(&dir).unwrap();

        let child1 = Command::new("echo")
            .arg("a")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let child2 = Command::new("echo")
            .arg("b")
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();

        registry.register("echo a", child1);
        registry.register("echo b", child2);

        std::thread::sleep(std::time::Duration::from_millis(100));
        let entries = registry.list();
        assert_eq!(entries.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn kill_terminates_running_process() {
        let dir = test_output_dir();
        let registry = ProcessRegistry::new(&dir).unwrap();

        // Spawn a long-running process.
        let child = Command::new("sleep")
            .arg("30")
            .stdout(std::process::Stdio::null())
            .spawn();

        // On Windows, `sleep` may not exist — skip gracefully.
        let Some(child) = child.ok() else {
            let _ = fs::remove_dir_all(&dir);
            return;
        };

        let id = registry.register("sleep 30", child);
        assert_eq!(registry.running_count(), 1);

        registry.kill(&id).expect("should kill");
        let entry = registry.status(&id).expect("should exist");
        assert_eq!(entry.status, ProcessStatus::Killed);
        assert_eq!(registry.running_count(), 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_process_returns_none() {
        let dir = test_output_dir();
        let registry = ProcessRegistry::new(&dir).unwrap();
        assert!(registry.status("bg-999").is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
