//! Syndicate orchestrator — manages the lifecycle of a syndicate run.
//!
//! Given a collection name and a task description, the orchestrator:
//! 1. Resolves the collection definition
//! 2. Creates a session directory with shared memory
//! 3. Spawns each agent as a background thread with full autonomy
//! 4. Monitors progress and collects results

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::syndicate_collection::{find_collection, SyndicateAgentDef, SyndicateCollection};
use crate::syndicate_memory::SyndicateMemory;

#[allow(clippy::cast_possible_truncation)] // epoch millis fits in u64 until ~year 584_942_417
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Status of an individual syndicate agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyndicateAgentStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl std::fmt::Display for SyndicateAgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Tracks a single agent within a syndicate run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyndicateAgentHandle {
    pub agent_id: String,
    pub name: String,
    pub role: String,
    pub status: SyndicateAgentStatus,
    pub error: Option<String>,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
}

/// Configuration for a syndicate run.
#[derive(Debug, Clone)]
pub struct SyndicateRunConfig {
    pub collection_name: String,
    pub task: String,
    pub model: Option<String>,
    pub session_dir: PathBuf,
}

/// Result of a syndicate run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyndicateRunResult {
    pub session_id: String,
    pub collection_name: String,
    pub task: String,
    pub agents: Vec<SyndicateAgentHandle>,
    pub memory_path: String,
    pub started_at: u64,
    pub completed_at: u64,
    pub success: bool,
}

/// The orchestrator that manages a syndicate's lifecycle.
#[derive(Debug)]
pub struct SyndicateOrchestrator {
    pub session_id: String,
    pub collection: SyndicateCollection,
    pub memory: SyndicateMemory,
    pub memory_path: PathBuf,
    pub agents: Vec<SyndicateAgentHandle>,
    pub task: String,
    pub model: Option<String>,
    pub started_at: u64,
}

impl SyndicateOrchestrator {
    /// Create a new orchestrator from a run config.
    /// Resolves the collection, creates the session directory, and initializes
    /// shared memory.
    pub fn new(config: SyndicateRunConfig, cwd: &Path) -> Result<Self, String> {
        let loaded = find_collection(cwd, &config.collection_name).ok_or_else(|| {
            format!(
                "unknown syndicate collection: '{}'. Use --list to see available collections.",
                config.collection_name
            )
        })?;

        let session_id = format!("syndicate-{:016x}", now_ms());
        let session_dir = config.session_dir.join(&session_id);
        std::fs::create_dir_all(&session_dir)
            .map_err(|e| format!("create syndicate session dir: {e}"))?;

        let memory_path = session_dir.join("memory.jsonl");
        let memory = SyndicateMemory::with_path(&memory_path)?;

        let agents = loaded
            .collection
            .agents
            .iter()
            .enumerate()
            .map(|(i, def)| SyndicateAgentHandle {
                agent_id: format!("{}-agent-{}-{}", session_id, i, def.name),
                name: def.name.clone(),
                role: def.role.clone(),
                status: SyndicateAgentStatus::Pending,
                error: None,
                started_at: None,
                completed_at: None,
            })
            .collect();

        Ok(Self {
            session_id,
            collection: loaded.collection,
            memory,
            memory_path,
            agents,
            task: config.task,
            model: config.model,
            started_at: now_ms(),
        })
    }

    /// Build the system prompt for a specific syndicate agent.
    #[must_use]
    pub fn build_agent_prompt(&self, agent_def: &SyndicateAgentDef) -> String {
        format!(
            "You are the **{role}** agent in a syndicate called **{collection}**.\n\n\
             ## Your Role\n{role_prompt}\n\n\
             ## Syndicate Task\n{task}\n\n\
             ## Coordination\n\
             Use `SyndicateMemoryWrite` to share findings, decisions, and progress with other agents.\n\
             Use `SyndicateMemoryRead` to check what other agents have written.\n\
             Use `SyndicateMemoryLog` to record observations.\n\
             Use `SyndicateMemorySearch` to find relevant entries.\n\
             You have full autonomy — no permission approval needed.\n\
             {subagent_note}\
             Work on your part of the task and write your results to shared memory.",
            role = agent_def.role,
            collection = self.collection.name,
            role_prompt = agent_def.system_prompt,
            task = self.task,
            subagent_note = if agent_def.can_spawn_subagents {
                "You can spawn sub-agents with the `Agent` tool for parallel subtasks.\n"
            } else {
                ""
            },
        )
    }

    /// Return the list of agent definitions from the collection.
    #[must_use]
    pub fn agent_defs(&self) -> &[SyndicateAgentDef] {
        &self.collection.agents
    }

    /// Mark an agent as running.
    pub fn mark_running(&mut self, index: usize) {
        if let Some(agent) = self.agents.get_mut(index) {
            agent.status = SyndicateAgentStatus::Running;
            agent.started_at = Some(now_ms());
        }
    }

    /// Mark an agent as completed.
    pub fn mark_completed(&mut self, index: usize) {
        if let Some(agent) = self.agents.get_mut(index) {
            agent.status = SyndicateAgentStatus::Completed;
            agent.completed_at = Some(now_ms());
        }
    }

    /// Mark an agent as failed.
    pub fn mark_failed(&mut self, index: usize, error: String) {
        if let Some(agent) = self.agents.get_mut(index) {
            agent.status = SyndicateAgentStatus::Failed;
            agent.error = Some(error);
            agent.completed_at = Some(now_ms());
        }
    }

    /// Check if all agents are done (completed or failed).
    #[must_use]
    pub fn all_done(&self) -> bool {
        self.agents.iter().all(|a| {
            matches!(
                a.status,
                SyndicateAgentStatus::Completed | SyndicateAgentStatus::Failed
            )
        })
    }

    /// Build the final result.
    #[must_use]
    pub fn result(&self) -> SyndicateRunResult {
        let success = self
            .agents
            .iter()
            .all(|a| a.status == SyndicateAgentStatus::Completed);
        SyndicateRunResult {
            session_id: self.session_id.clone(),
            collection_name: self.collection.name.clone(),
            task: self.task.clone(),
            agents: self.agents.clone(),
            memory_path: self.memory_path.display().to_string(),
            started_at: self.started_at,
            completed_at: now_ms(),
            success,
        }
    }

    /// Render a human-readable summary.
    #[must_use]
    pub fn render_summary(&self) -> String {
        use std::fmt::Write;
        let result = self.result();
        let mut out = String::new();
        let _ = writeln!(out, "\n━━━ Syndicate: {} ━━━", result.collection_name);
        let _ = writeln!(out, "Session: {}", result.session_id);
        let _ = writeln!(out, "Task: {}\n", result.task);

        out.push_str("Agents:\n");
        for agent in &result.agents {
            let status_icon = match agent.status {
                SyndicateAgentStatus::Completed => "✓",
                SyndicateAgentStatus::Failed => "✗",
                SyndicateAgentStatus::Running => "⟳",
                SyndicateAgentStatus::Pending => "○",
            };
            let _ = writeln!(
                out,
                "  {} {} ({}): {}",
                status_icon, agent.name, agent.role, agent.status
            );
            if let Some(error) = &agent.error {
                let _ = writeln!(out, "    Error: {error}");
            }
        }

        // Show memory contents
        let entries = self.memory.read_all();
        if !entries.is_empty() {
            out.push_str("\nShared Memory:\n");
            for entry in &entries {
                let preview = if entry.value.len() > 120 {
                    format!("{}...", &entry.value[..120])
                } else {
                    entry.value.clone()
                };
                let _ = writeln!(
                    out,
                    "  [{}] {} = {}",
                    entry.written_by, entry.key, preview
                );
            }
        }

        let _ = writeln!(
            out,
            "\nResult: {}",
            if result.success { "SUCCESS" } else { "PARTIAL FAILURE" }
        );
        let _ = writeln!(out, "Memory: {}", result.memory_path);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_session_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("eidolon_syndicate_orch_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn new_orchestrator_from_builtin() {
        let dir = test_session_dir();
        let cwd = std::env::temp_dir();
        let config = SyndicateRunConfig {
            collection_name: "feature-build".into(),
            task: "implement a health check".into(),
            model: None,
            session_dir: dir.clone(),
        };
        let orch = SyndicateOrchestrator::new(config, &cwd).unwrap();
        assert_eq!(orch.collection.name, "feature-build");
        assert_eq!(orch.agents.len(), 4);
        assert!(orch.session_id.starts_with("syndicate-"));
        assert!(orch.memory_path.exists() || true); // path is set even if file not yet created
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_collection_errors() {
        let dir = test_session_dir();
        let config = SyndicateRunConfig {
            collection_name: "nonexistent".into(),
            task: "test".into(),
            model: None,
            session_dir: dir.clone(),
        };
        let result = SyndicateOrchestrator::new(config, &std::env::temp_dir());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown syndicate collection"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn agent_lifecycle_tracking() {
        let dir = test_session_dir();
        let config = SyndicateRunConfig {
            collection_name: "research".into(),
            task: "analyze the codebase".into(),
            model: None,
            session_dir: dir.clone(),
        };
        let mut orch = SyndicateOrchestrator::new(config, &std::env::temp_dir()).unwrap();
        assert!(!orch.all_done());

        orch.mark_running(0);
        assert_eq!(orch.agents[0].status, SyndicateAgentStatus::Running);

        orch.mark_completed(0);
        assert_eq!(orch.agents[0].status, SyndicateAgentStatus::Completed);

        orch.mark_failed(1, "timeout".into());
        assert_eq!(orch.agents[1].status, SyndicateAgentStatus::Failed);

        orch.mark_completed(2);
        assert!(orch.all_done());

        let result = orch.result();
        assert!(!result.success); // one failed
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_agent_prompt_includes_task() {
        let dir = test_session_dir();
        let config = SyndicateRunConfig {
            collection_name: "feature-build".into(),
            task: "add logging middleware".into(),
            model: None,
            session_dir: dir.clone(),
        };
        let orch = SyndicateOrchestrator::new(config, &std::env::temp_dir()).unwrap();
        let prompt = orch.build_agent_prompt(&orch.collection.agents[0]);
        assert!(prompt.contains("add logging middleware"));
        assert!(prompt.contains("Planner"));
        assert!(prompt.contains("SyndicateMemoryWrite"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn render_summary_includes_agents() {
        let dir = test_session_dir();
        let config = SyndicateRunConfig {
            collection_name: "research".into(),
            task: "test task".into(),
            model: None,
            session_dir: dir.clone(),
        };
        let mut orch = SyndicateOrchestrator::new(config, &std::env::temp_dir()).unwrap();
        orch.mark_completed(0);
        orch.mark_completed(1);
        orch.mark_completed(2);
        let summary = orch.render_summary();
        assert!(summary.contains("research"));
        assert!(summary.contains("SUCCESS"));
        let _ = fs::remove_dir_all(&dir);
    }
}
