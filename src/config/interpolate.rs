use super::*;

/// Remplace les occurrences `{{ var }}` dans `s` par les valeurs de `vars`.
/// Les variables non définies sont laissées telles quelles (`{{ var }}`).
pub fn interpolate(s: &str, vars: &HashMap<String, String>) -> String {
    let mut result = s.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{ {key} }}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

/// Retourne les noms des variables `{{ var }}` présentes dans `s` mais absentes de `vars`.
pub fn undefined_vars(s: &str, vars: &HashMap<String, String>) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find("}}") {
            let inner = rest[..end].trim();
            if !inner.is_empty() && !vars.contains_key(inner) {
                found.push(inner.to_string());
            }
            rest = &rest[end + 2..];
        } else {
            break;
        }
    }
    found
}

/// Les tags s'accumulent : chaque niveau **ajoute** ses tags à ceux du niveau parent.
/// Un serveur hérite donc des tags définis dans les defaults, le groupe et l'environnement.
pub(crate) fn extend_tags(
    parent: Option<&Vec<String>>,
    child: Option<&Vec<String>>,
) -> Vec<String> {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut merged: Vec<String> = Vec::new();
    for tag in parent
        .into_iter()
        .flatten()
        .chain(child.into_iter().flatten())
    {
        if seen.insert(tag.as_str()) {
            merged.push(tag.clone());
        }
    }
    merged
}

/// Les probe_filesystems s'accumulent en cascade : chaque niveau ajoute ses
/// entrées à celles du niveau parent (sans doublon).
/// Un groupe définissant `/kafka_data` héritera donc aussi des filesystems
/// déclarés dans les defaults.
pub(crate) fn extend_filesystems(
    parent: Option<&Vec<String>>,
    child: Option<&Vec<String>>,
) -> Option<Vec<String>> {
    if parent.is_none() && child.is_none() {
        return None;
    }
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut merged: Vec<String> = Vec::new();
    for item in parent
        .into_iter()
        .flatten()
        .chain(child.into_iter().flatten())
    {
        if seen.insert(item.as_str()) {
            merged.push(item.clone());
        }
    }
    Some(merged)
}

/// Fusionne deux `Defaults` : `overrides` prime sur `base` pour chaque champ `Option`.
/// Utilisé par `load_merged` quand `merge_defaults: true` est activé sur un include.
pub(crate) fn merge_default_structs(base: &Defaults, overrides: &Defaults) -> Defaults {
    Defaults {
        user: overrides.user.clone().or_else(|| base.user.clone()),
        ssh_key: overrides.ssh_key.clone().or_else(|| base.ssh_key.clone()),
        ssh_cert: overrides.ssh_cert.clone().or_else(|| base.ssh_cert.clone()),
        ssh_agent_sock: overrides
            .ssh_agent_sock
            .clone()
            .or_else(|| base.ssh_agent_sock.clone()),
        mode: overrides.mode.or(base.mode),
        ssh_port: overrides.ssh_port.or(base.ssh_port),
        ssh_options: overrides
            .ssh_options
            .clone()
            .or_else(|| base.ssh_options.clone()),
        wallix: overrides.wallix.clone().or_else(|| base.wallix.clone()),
        jump: overrides.jump.clone().or_else(|| base.jump.clone()),
        use_system_ssh_config: overrides
            .use_system_ssh_config
            .or(base.use_system_ssh_config),
        theme: overrides.theme.or(base.theme),
        probe_filesystems: overrides
            .probe_filesystems
            .clone()
            .or_else(|| base.probe_filesystems.clone()),
        keep_open: overrides.keep_open.or(base.keep_open),
        tunnels: overrides.tunnels.clone().or_else(|| base.tunnels.clone()),
        default_filter: overrides
            .default_filter
            .clone()
            .or_else(|| base.default_filter.clone()),
        tags: match (&base.tags, &overrides.tags) {
            (None, r) => r.clone(),
            (l, None) => l.clone(),
            (Some(b), Some(o)) => Some(extend_tags(Some(b), Some(o))),
        },
        control_master: overrides.control_master.or(base.control_master),
        agent_forwarding: overrides.agent_forwarding.or(base.agent_forwarding),
        control_path: overrides
            .control_path
            .clone()
            .or_else(|| base.control_path.clone()),
        control_persist: overrides
            .control_persist
            .clone()
            .or_else(|| base.control_persist.clone()),
        pre_connect_hook: overrides
            .pre_connect_hook
            .clone()
            .or_else(|| base.pre_connect_hook.clone()),
        post_disconnect_hook: overrides
            .post_disconnect_hook
            .clone()
            .or_else(|| base.post_disconnect_hook.clone()),
        hook_timeout_secs: overrides.hook_timeout_secs.or(base.hook_timeout_secs),
    }
}
