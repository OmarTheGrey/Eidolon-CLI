//! Profile isolation — run multiple Eidolon instances with separate config,
//! sessions, skills, and credentials from a single installation.
//!
//! Each profile lives under `~/.eidolon/profiles/<name>/` and contains its own
//! config, sessions, skills, and credentials. The active profile is selected
//! via the `EIDOLON_PROFILE` env var or `--profile <name>` CLI flag.

use std::fs;
use std::path::PathBuf;

/// Returns the base Eidolon home directory (without profile override).
/// This is always `~/.eidolon` or `$EIDOLON_CONFIG_HOME` if set.
#[must_use]
pub fn eidolon_base_home() -> PathBuf {
    std::env::var_os("EIDOLON_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".eidolon")))
        .or_else(|| {
            std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".eidolon"))
        })
        .unwrap_or_else(|| PathBuf::from(".eidolon"))
}

/// Returns the profiles root directory (`~/.eidolon/profiles/`).
#[must_use]
pub fn profiles_dir() -> PathBuf {
    eidolon_base_home().join("profiles")
}

/// Returns the config home for the active profile. If `EIDOLON_PROFILE` is
/// set, returns `~/.eidolon/profiles/<name>/`. Otherwise returns the default
/// config home.
#[must_use]
pub fn resolve_profile_home() -> PathBuf {
    if let Some(profile) = active_profile_name() {
        profiles_dir().join(profile)
    } else {
        eidolon_base_home()
    }
}

/// Returns the active profile name, if one is set via `EIDOLON_PROFILE`.
#[must_use]
pub fn active_profile_name() -> Option<String> {
    std::env::var("EIDOLON_PROFILE")
        .ok()
        .filter(|name| !name.is_empty())
}

/// List all available profiles.
#[must_use]
pub fn list_profiles() -> Vec<String> {
    let dir = profiles_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|ft| ft.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

/// Create a new profile directory with default structure.
pub fn create_profile(name: &str) -> Result<PathBuf, String> {
    validate_profile_name(name)?;
    let profile_dir = profiles_dir().join(name);
    if profile_dir.exists() {
        return Err(format!("profile '{name}' already exists"));
    }
    fs::create_dir_all(profile_dir.join("sessions"))
        .map_err(|e| format!("failed to create profile directory: {e}"))?;
    fs::create_dir_all(profile_dir.join("skills"))
        .map_err(|e| format!("failed to create profile skills directory: {e}"))?;
    Ok(profile_dir)
}

/// Delete a profile directory.
pub fn delete_profile(name: &str) -> Result<(), String> {
    validate_profile_name(name)?;
    let profile_dir = profiles_dir().join(name);
    if !profile_dir.exists() {
        return Err(format!("profile '{name}' does not exist"));
    }
    fs::remove_dir_all(&profile_dir)
        .map_err(|e| format!("failed to delete profile '{name}': {e}"))?;
    Ok(())
}

/// Check that a profile path exists.
#[must_use]
pub fn profile_exists(name: &str) -> bool {
    profiles_dir().join(name).is_dir()
}

fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("profile name cannot be empty".to_string());
    }
    if name.contains(std::path::MAIN_SEPARATOR)
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
    {
        return Err(format!(
            "profile name '{name}' contains invalid characters"
        ));
    }
    if name.len() > 64 {
        return Err("profile name must be 64 characters or shorter".to_string());
    }
    Ok(())
}

/// Apply a profile override early in process startup. Sets `EIDOLON_CONFIG_HOME`
/// so that all subsequent `default_config_home()` calls resolve to the profile.
pub fn apply_profile_override(profile_name: &str) -> Result<(), String> {
    validate_profile_name(profile_name)?;
    let profile_home = profiles_dir().join(profile_name);
    if !profile_home.exists() {
        return Err(format!(
            "profile '{profile_name}' does not exist. Create it with `eidolon-cli profile create {profile_name}`"
        ));
    }
    std::env::set_var("EIDOLON_CONFIG_HOME", &profile_home);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_path_traversal() {
        assert!(validate_profile_name("../escape").is_err());
        assert!(validate_profile_name("a/b").is_err());
        assert!(validate_profile_name("").is_err());
    }

    #[test]
    fn validate_accepts_simple_names() {
        assert!(validate_profile_name("work").is_ok());
        assert!(validate_profile_name("my-project-2").is_ok());
        assert!(validate_profile_name("personal").is_ok());
    }

    #[test]
    fn create_and_delete_profile_round_trips() {
        let name = "eidolon-test-profile-roundtrip";
        // Clean up from any previous run
        let _ = delete_profile(name);

        let path = create_profile(name).expect("should create profile");
        assert!(path.exists());
        assert!(path.join("sessions").is_dir());
        assert!(path.join("skills").is_dir());

        assert!(profile_exists(name));
        assert!(list_profiles().contains(&name.to_string()));

        // Duplicate should fail
        assert!(create_profile(name).is_err());

        delete_profile(name).expect("should delete profile");
        assert!(!profile_exists(name));
    }
}
