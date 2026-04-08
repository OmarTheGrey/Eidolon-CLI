//! Syndicate collection definitions.
//!
//! A *collection* is a named group of agent roles that form a syndicate.
//! Collections can be defined as TOML files in `.eidolon/syndicates/` (project
//! or user-level) or use one of the built-in defaults compiled into the binary.

use std::env;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A single agent role within a syndicate collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyndicateAgentDef {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub system_prompt: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default = "default_true")]
    pub can_spawn_subagents: bool,
}

fn default_true() -> bool {
    true
}

/// A named collection of agent roles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyndicateCollection {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub agents: Vec<SyndicateAgentDef>,
}

/// Where a collection was loaded from.
#[derive(Debug, Clone)]
pub enum CollectionSource {
    Builtin,
    Project(PathBuf),
    User(PathBuf),
}

/// A loaded collection with provenance.
#[derive(Debug, Clone)]
pub struct LoadedCollection {
    pub collection: SyndicateCollection,
    pub source: CollectionSource,
}

// ── Built-in collections ────────────────────────────────────────────────────

fn builtin_feature_build() -> SyndicateCollection {
    SyndicateCollection {
        name: "feature-build".into(),
        description: "Plan, implement, review, and test a feature end-to-end.".into(),
        agents: vec![
            SyndicateAgentDef {
                name: "planner".into(),
                role: "Planner".into(),
                system_prompt: concat!(
                    "You are the Planner agent. Analyze the task, break it into subtasks, ",
                    "identify affected files, and write a structured plan to shared memory. ",
                    "Use SyndicateMemoryWrite to store the plan under key 'plan'. ",
                    "Use SyndicateMemoryLog to record your reasoning."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: true,
            },
            SyndicateAgentDef {
                name: "implementer".into(),
                role: "Implementer".into(),
                system_prompt: concat!(
                    "You are the Implementer agent. Read the plan from shared memory ",
                    "(key 'plan') and execute it. Write code, create files, and run tests. ",
                    "Update shared memory with progress under key 'implementation_status'. ",
                    "Spawn subagents for parallel subtasks when beneficial."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: true,
            },
            SyndicateAgentDef {
                name: "reviewer".into(),
                role: "Reviewer".into(),
                system_prompt: concat!(
                    "You are the Reviewer agent. Read the implementation status from shared ",
                    "memory and review all changes. Check for correctness, style, edge cases, ",
                    "and security issues. Write findings to shared memory under key 'review'. ",
                    "Log specific file-level observations."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: false,
            },
            SyndicateAgentDef {
                name: "tester".into(),
                role: "Tester".into(),
                system_prompt: concat!(
                    "You are the Tester agent. Read the plan and implementation from shared ",
                    "memory, then write and run tests. Verify the acceptance criteria are met. ",
                    "Write test results to shared memory under key 'test_results'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: true,
            },
        ],
    }
}

fn builtin_research() -> SyndicateCollection {
    SyndicateCollection {
        name: "research".into(),
        description: "Explore, analyze, and synthesize findings about a topic.".into(),
        agents: vec![
            SyndicateAgentDef {
                name: "explorer".into(),
                role: "Explorer".into(),
                system_prompt: concat!(
                    "You are the Explorer agent. Search the codebase and documentation ",
                    "thoroughly. Read relevant files, search for patterns, and gather raw ",
                    "findings. Write discoveries to shared memory with descriptive keys."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: true,
            },
            SyndicateAgentDef {
                name: "analyst".into(),
                role: "Analyst".into(),
                system_prompt: concat!(
                    "You are the Analyst agent. Read the explorer's findings from shared ",
                    "memory and perform deeper analysis. Identify patterns, dependencies, ",
                    "trade-offs, and risks. Write analysis to shared memory under key 'analysis'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: false,
            },
            SyndicateAgentDef {
                name: "synthesizer".into(),
                role: "Synthesizer".into(),
                system_prompt: concat!(
                    "You are the Synthesizer agent. Read all findings and analysis from ",
                    "shared memory and produce a clear, actionable summary. Write the final ",
                    "synthesis to shared memory under key 'synthesis'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: false,
            },
        ],
    }
}

fn builtin_code_review() -> SyndicateCollection {
    SyndicateCollection {
        name: "code-review".into(),
        description: "Multi-perspective code review: correctness, security, and documentation."
            .into(),
        agents: vec![
            SyndicateAgentDef {
                name: "reviewer".into(),
                role: "Code Reviewer".into(),
                system_prompt: concat!(
                    "You are the Code Reviewer agent. Review the code for correctness, ",
                    "logic errors, edge cases, and adherence to project conventions. ",
                    "Write findings to shared memory under key 'code_review'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: false,
            },
            SyndicateAgentDef {
                name: "security-auditor".into(),
                role: "Security Auditor".into(),
                system_prompt: concat!(
                    "You are the Security Auditor agent. Review the code for security ",
                    "vulnerabilities: injection, access control, cryptographic issues, ",
                    "SSRF, and other OWASP Top 10 concerns. Write findings to shared ",
                    "memory under key 'security_audit'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: false,
            },
            SyndicateAgentDef {
                name: "docs-checker".into(),
                role: "Documentation Checker".into(),
                system_prompt: concat!(
                    "You are the Documentation Checker agent. Verify that code changes ",
                    "have adequate documentation, comments where needed, and that public ",
                    "APIs are properly documented. Write findings to shared memory under ",
                    "key 'docs_review'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: false,
            },
        ],
    }
}

fn builtin_refactor() -> SyndicateCollection {
    SyndicateCollection {
        name: "refactor".into(),
        description: "Analyze, refactor, and verify code changes.".into(),
        agents: vec![
            SyndicateAgentDef {
                name: "analyzer".into(),
                role: "Analyzer".into(),
                system_prompt: concat!(
                    "You are the Analyzer agent. Study the code to be refactored. Map ",
                    "dependencies, identify coupling points, and assess risk. Write the ",
                    "analysis to shared memory under key 'refactor_analysis'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: true,
            },
            SyndicateAgentDef {
                name: "implementer".into(),
                role: "Refactor Implementer".into(),
                system_prompt: concat!(
                    "You are the Refactor Implementer agent. Read the analysis from shared ",
                    "memory and execute the refactoring. Make changes incrementally. Update ",
                    "shared memory with progress under key 'refactor_status'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: true,
            },
            SyndicateAgentDef {
                name: "verifier".into(),
                role: "Verifier".into(),
                system_prompt: concat!(
                    "You are the Verifier agent. After refactoring, run the test suite, ",
                    "check for regressions, and verify the refactoring preserved behavior. ",
                    "Write verification results to shared memory under key 'verification'."
                )
                .into(),
                model: None,
                tools: vec![],
                can_spawn_subagents: true,
            },
        ],
    }
}

/// All built-in collections.
#[must_use]
pub fn builtin_collections() -> Vec<SyndicateCollection> {
    vec![
        builtin_feature_build(),
        builtin_research(),
        builtin_code_review(),
        builtin_refactor(),
    ]
}

// ── Discovery ────────────────────────────────────────────────────────────────

/// Discover syndicate collection files from project and user directories.
/// Returns collections ordered by precedence (project overrides user overrides builtin).
#[must_use]
pub fn discover_collections(cwd: &Path) -> Vec<LoadedCollection> {
    let mut loaded: Vec<LoadedCollection> = Vec::new();
    let mut seen_names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // Project hierarchy — walk up ancestors
    for ancestor in cwd.ancestors() {
        load_collections_from_dir(
            &ancestor.join(".eidolon").join("syndicates"),
            &CollectionSource::Project(ancestor.to_path_buf()),
            &mut loaded,
            &mut seen_names,
        );
    }

    // User home directories
    let home_dirs = resolve_home_dirs();
    for home in &home_dirs {
        load_collections_from_dir(
            &home.join(".eidolon").join("syndicates"),
            &CollectionSource::User(home.clone()),
            &mut loaded,
            &mut seen_names,
        );
    }

    // Built-in defaults (lowest precedence)
    for collection in builtin_collections() {
        if !seen_names.contains(&collection.name) {
            seen_names.insert(collection.name.clone());
            loaded.push(LoadedCollection {
                collection,
                source: CollectionSource::Builtin,
            });
        }
    }

    loaded
}

/// Find a specific collection by name.
#[must_use]
pub fn find_collection(cwd: &Path, name: &str) -> Option<LoadedCollection> {
    discover_collections(cwd)
        .into_iter()
        .find(|lc| lc.collection.name == name)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn load_collections_from_dir(
    dir: &Path,
    source: &CollectionSource,
    loaded: &mut Vec<LoadedCollection>,
    seen: &mut std::collections::BTreeSet<String>,
) {
    if !dir.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        match load_collection_file(&path) {
            Ok(collection) => {
                if !seen.contains(&collection.name) {
                    seen.insert(collection.name.clone());
                    loaded.push(LoadedCollection {
                        collection,
                        source: source.clone(),
                    });
                }
            }
            Err(e) => {
                eprintln!("syndicate: skipping {}: {e}", path.display());
            }
        }
    }
}

fn load_collection_file(path: &Path) -> Result<SyndicateCollection, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    toml_parse(&contents)
}

/// Minimal TOML parser for syndicate collection files.
///
/// Supports the subset needed for collection definitions:
/// - Top-level `name` and `description` string fields.
/// - `[[agents]]` array-of-tables with string, boolean, and string-array values.
///
/// **Known limitations** (intentional — avoids a full TOML dependency):
/// - No multi-line strings (basic or literal).
/// - No escape sequences inside quoted strings.
/// - No inline tables or dotted keys.
/// - No nested arrays beyond the simple `["a", "b"]` form used by `tools`.
/// - Integer and float values are not parsed; everything is treated as a string.
fn toml_parse(input: &str) -> Result<SyndicateCollection, String> {
    // We parse manually rather than adding a toml dependency, since the format
    // is simple and the runtime crate doesn't have toml in its deps.
    let mut name: Option<String> = None;
    let mut description = String::new();
    let mut agents: Vec<SyndicateAgentDef> = Vec::new();
    let mut current_agent: Option<AgentBuilder> = None;

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "[[agents]]" {
            if let Some(builder) = current_agent.take() {
                agents.push(builder.build()?);
            }
            current_agent = Some(AgentBuilder::default());
            continue;
        }

        if let Some((key, value)) = parse_kv(trimmed) {
            if let Some(agent) = current_agent.as_mut() {
                match key {
                    "name" => agent.name = Some(unquote(value)),
                    "role" => agent.role = Some(unquote(value)),
                    "system_prompt" => agent.system_prompt = Some(unquote(value)),
                    "model" => agent.model = Some(unquote(value)),
                    "can_spawn_subagents" => {
                        agent.can_spawn_subagents = Some(value == "true");
                    }
                    "tools" => {
                        agent.tools = parse_string_array(value);
                    }
                    _ => {}
                }
            } else {
                match key {
                    "name" => name = Some(unquote(value)),
                    "description" => description = unquote(value),
                    _ => {}
                }
            }
        }
    }

    // Flush last agent
    if let Some(builder) = current_agent.take() {
        agents.push(builder.build()?);
    }

    let name = name.ok_or_else(|| "missing 'name' field in collection".to_string())?;
    if agents.is_empty() {
        return Err(format!("collection '{name}' has no [[agents]]"));
    }

    Ok(SyndicateCollection {
        name,
        description,
        agents,
    })
}

#[derive(Default)]
struct AgentBuilder {
    name: Option<String>,
    role: Option<String>,
    system_prompt: Option<String>,
    model: Option<String>,
    tools: Vec<String>,
    can_spawn_subagents: Option<bool>,
}

impl AgentBuilder {
    fn build(self) -> Result<SyndicateAgentDef, String> {
        let name = self
            .name
            .ok_or_else(|| "agent missing 'name'".to_string())?;
        let role = self.role.unwrap_or_else(|| name.clone());
        Ok(SyndicateAgentDef {
            name,
            role,
            system_prompt: self.system_prompt.unwrap_or_default(),
            model: self.model,
            tools: self.tools,
            can_spawn_subagents: self.can_spawn_subagents.unwrap_or(true),
        })
    }
}

fn parse_kv(line: &str) -> Option<(&str, &str)> {
    let eq = line.find('=')?;
    let key = line[..eq].trim();
    let value = line[eq + 1..].trim();
    Some((key, value))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_string_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return vec![];
    }
    s[1..s.len() - 1]
        .split(',')
        .map(|item| unquote(item.trim()))
        .filter(|item| !item.is_empty())
        .collect()
}

fn resolve_home_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = env::var("HOME") {
        dirs.push(PathBuf::from(home));
    }
    #[cfg(windows)]
    if let Ok(profile) = env::var("USERPROFILE") {
        let p = PathBuf::from(profile);
        if !dirs.contains(&p) {
            dirs.push(p);
        }
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_collection() {
        let toml = r#"
name = "test-collection"
description = "A test"

[[agents]]
name = "worker"
role = "Worker"
system_prompt = "Do work."
"#;
        let c = toml_parse(toml).unwrap();
        assert_eq!(c.name, "test-collection");
        assert_eq!(c.agents.len(), 1);
        assert_eq!(c.agents[0].name, "worker");
        assert_eq!(c.agents[0].role, "Worker");
        assert!(c.agents[0].can_spawn_subagents);
    }

    #[test]
    fn parse_multi_agent_collection() {
        let toml = r#"
name = "duo"
description = "Two agents"

[[agents]]
name = "alpha"
role = "Lead"
system_prompt = "You lead."
can_spawn_subagents = false

[[agents]]
name = "beta"
role = "Support"
system_prompt = "You support."
tools = ["bash", "read_file"]
"#;
        let c = toml_parse(toml).unwrap();
        assert_eq!(c.agents.len(), 2);
        assert!(!c.agents[0].can_spawn_subagents);
        assert!(c.agents[1].can_spawn_subagents);
        assert_eq!(c.agents[1].tools, vec!["bash", "read_file"]);
    }

    #[test]
    fn parse_missing_name_errors() {
        let toml = r#"
description = "no name"

[[agents]]
name = "a"
"#;
        assert!(toml_parse(toml).is_err());
    }

    #[test]
    fn parse_no_agents_errors() {
        let toml = r#"
name = "empty"
"#;
        assert!(toml_parse(toml).is_err());
    }

    #[test]
    fn builtin_collections_valid() {
        let collections = builtin_collections();
        assert!(collections.len() >= 4);
        for c in &collections {
            assert!(!c.name.is_empty());
            assert!(!c.agents.is_empty());
        }
    }

    #[test]
    fn discover_includes_builtins() {
        // When run from a temp dir with no .eidolon/, builtins are still found
        let dir = std::env::temp_dir().join("eidolon_syndicate_discover_test");
        let _ = std::fs::create_dir_all(&dir);
        let loaded = discover_collections(&dir);
        assert!(loaded.len() >= 4);
        let names: Vec<&str> = loaded.iter().map(|l| l.collection.name.as_str()).collect();
        assert!(names.contains(&"feature-build"));
        assert!(names.contains(&"research"));
        assert!(names.contains(&"code-review"));
        assert!(names.contains(&"refactor"));
    }
}
