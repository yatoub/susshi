use super::*;

/// Represents a single entry from a Wallix menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallixMenuEntry {
    /// Menu entry ID (as String to preserve leading zeros).
    pub id: String,
    /// Target in format "user@account@host:protocol".
    pub target: String,
    /// Authorization group name.
    pub group: String,
}

/// Build the expected Wallix target string from a resolved server.
///
/// Format: `user@account@host:protocol`
pub fn build_expected_target(server: &ResolvedServer) -> String {
    format!(
        "{}@{}@{}:{}",
        server.user, server.wallix_account, server.host, server.wallix_protocol
    )
}

fn normalize_target_segment(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_dash = false;

    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_uppercase());
            previous_was_dash = false;
        } else if !previous_was_dash {
            normalized.push('-');
            previous_was_dash = true;
        }
    }

    normalized.trim_matches('-').to_string()
}

fn infer_wallix_role(server: &ResolvedServer) -> Option<String> {
    let first_host_label = server
        .host
        .split('.')
        .next()
        .unwrap_or(server.host.as_str());
    let host_last_token = first_host_label
        .rsplit('-')
        .next()
        .unwrap_or(first_host_label);
    let source = if !server.name.trim().is_empty() {
        server.name.as_str()
    } else {
        host_last_token
    };

    let role_key: String = source
        .chars()
        .take_while(|character| !character.is_ascii_digit())
        .collect::<String>()
        .to_ascii_lowercase();

    let role = match role_key.as_str() {
        "bdd" | "bd" | "db" => "BD",
        "apps" | "app" | "appli" => "APPLI",
        "adm" | "admin" => "ADMIN",
        "web" => "WEB",
        "kafka" => "KAFKA",
        "els" => "ELS",
        "mig" => "MIG",
        "idp" => "IDP",
        "frt" | "frtrac" => "FRTRAC",
        other if !other.is_empty() => return Some(normalize_target_segment(other)),
        _ => return None,
    };

    Some(role.to_string())
}

pub fn build_expected_targets(server: &ResolvedServer) -> Vec<String> {
    let mut candidates = vec![build_expected_target(server)];

    let first_host_label = server
        .host
        .split('.')
        .next()
        .unwrap_or(server.host.as_str());
    let short_host = normalize_target_segment(first_host_label);
    if !short_host.is_empty() {
        candidates.push(format!(
            "{}@{}@{}:{}",
            server.user, server.wallix_account, short_host, server.wallix_protocol
        ));
    }

    let env = if !server.env_name.trim().is_empty() {
        normalize_target_segment(&server.env_name)
    } else {
        normalize_target_segment(first_host_label.split('-').next().unwrap_or_default())
    };

    let project_from_domain = server.host.split('.').nth(1).map(normalize_target_segment);
    let project_from_group = if server.group_name.trim().is_empty() {
        String::new()
    } else {
        normalize_target_segment(&server.group_name)
    };

    let role = infer_wallix_role(server).unwrap_or_default();

    for project in [project_from_domain.unwrap_or_default(), project_from_group] {
        if !env.is_empty() && !project.is_empty() && !role.is_empty() {
            candidates.push(format!(
                "{}@{}@{}-{}-{}:{}",
                server.user, server.wallix_account, env, project, role, server.wallix_protocol
            ));
        }
    }

    candidates.dedup();
    candidates
}

fn normalize_authorization_segment(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_dash = false;

    for character in value.trim().chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_uppercase());
            previous_was_dash = false;
        } else if !previous_was_dash {
            normalized.push('-');
            previous_was_dash = true;
        }
    }

    normalized.trim_matches('-').to_string()
}

pub fn build_expected_groups(server: &ResolvedServer) -> Result<Vec<String>> {
    let configured_group = match server
        .wallix_group
        .as_deref()
        .map(str::trim)
        .filter(|g| !g.is_empty())
    {
        Some(g) => g,
        None => return Ok(vec![]),
    };

    let mut candidates = vec![configured_group.to_string()];

    if !configured_group.contains('_') {
        let mut prefix_parts = Vec::new();
        if !server.env_name.trim().is_empty() {
            let env = normalize_authorization_segment(&server.env_name);
            if !env.is_empty() {
                prefix_parts.push(env);
            }
        }
        if !server.group_name.trim().is_empty() {
            let group = normalize_authorization_segment(&server.group_name);
            if !group.is_empty() {
                prefix_parts.push(group);
            }
        }

        if !prefix_parts.is_empty() {
            candidates.push(format!("{}_{}", prefix_parts.join("-"), configured_group));
        }
    }

    candidates.dedup();
    Ok(candidates)
}

pub(crate) fn group_suffix_matches(entry_group: &str, configured_group: &str) -> bool {
    let entry_lower = entry_group.to_ascii_lowercase();
    let conf_lower = configured_group.to_ascii_lowercase();
    entry_lower == conf_lower || entry_lower.ends_with(&format!("_{conf_lower}"))
}
