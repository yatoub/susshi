use super::*;

// ─── Validation YAML ─────────────────────────────────────────────────────────

/// Télécharge le contenu d'une URL HTTPS et le retourne sous forme de `String`.
/// Rejette les URLs HTTP non-chiffrées. Timeout global : `HTTP_TIMEOUT_SECS`.
/// Le corps est limité à 10 Mo par défaut (limite ureq 3.x sur `read_to_string`).
pub(crate) fn fetch_url(url: &str) -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .timeout_global(Some(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS)))
        .build()
        .into();
    agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())
}

/// Analyse `content` (YAML texte) et retourne les avertissements pour tout champ
/// dont le nom ne figure pas dans la liste des clés connues du schéma susshi.
pub fn validate_yaml(content: &str, file_path: &str) -> Vec<ValidationWarning> {
    let value: serde_yaml_ng::Value = match serde_yaml_ng::from_str(content) {
        Ok(v) => v,
        Err(_) => return vec![], // l'erreur de parsing est déjà remontée par serde
    };

    let mut warnings = Vec::new();

    if let serde_yaml_ng::Value::Mapping(root) = &value {
        yaml_check_keys(
            root,
            &["defaults", "groups", "includes", "_vars"],
            file_path,
            "root",
            &mut warnings,
        );

        if let Some(serde_yaml_ng::Value::Mapping(m)) = root.get("defaults") {
            yaml_check_keys(
                m,
                &[
                    "user",
                    "ssh_key",
                    "mode",
                    "ssh_port",
                    "ssh_options",
                    "wallix",
                    "jump",
                    "use_system_ssh_config",
                    "theme",
                    "probe_filesystems",
                    "keep_open",
                    "tunnels",
                    "default_filter",
                    "tags",
                    "control_master",
                    "control_path",
                    "control_persist",
                    "pre_connect_hook",
                    "post_disconnect_hook",
                    "hook_timeout_secs",
                ],
                file_path,
                "defaults",
                &mut warnings,
            );
        }

        if let Some(serde_yaml_ng::Value::Sequence(incs)) = root.get("includes") {
            for (i, inc) in incs.iter().enumerate() {
                if let serde_yaml_ng::Value::Mapping(m) = inc {
                    yaml_check_keys(
                        m,
                        &["label", "path", "merge_defaults"],
                        file_path,
                        &format!("includes[{i}]"),
                        &mut warnings,
                    );
                }
            }
        }

        if let Some(serde_yaml_ng::Value::Sequence(groups)) = root.get("groups") {
            for (i, g) in groups.iter().enumerate() {
                yaml_validate_entry(g, file_path, &format!("groups[{i}]"), &mut warnings);
            }
        }
    }

    warnings
}

fn yaml_validate_entry(
    val: &serde_yaml_ng::Value,
    file: &str,
    ctx: &str,
    warnings: &mut Vec<ValidationWarning>,
) {
    let serde_yaml_ng::Value::Mapping(m) = val else {
        return;
    };
    let has_host = m.contains_key(serde_yaml_ng::Value::String("host".into()));
    let has_envs = m.contains_key(serde_yaml_ng::Value::String("environments".into()));

    if has_host && !has_envs {
        // Serveur
        yaml_check_keys(
            m,
            &[
                "name",
                "host",
                "user",
                "ssh_key",
                "ssh_port",
                "ssh_options",
                "mode",
                "wallix",
                "wallix_group",
                "jump",
                "probe_filesystems",
                "tunnels",
                "tags",
            ],
            file,
            ctx,
            warnings,
        );
    } else {
        // Groupe
        yaml_check_keys(
            m,
            &[
                "name",
                "user",
                "ssh_key",
                "mode",
                "ssh_port",
                "ssh_options",
                "wallix",
                "wallix_group",
                "jump",
                "environments",
                "servers",
                "probe_filesystems",
                "tunnels",
                "tags",
            ],
            file,
            ctx,
            warnings,
        );

        if let Some(serde_yaml_ng::Value::Sequence(envs)) =
            m.get(serde_yaml_ng::Value::String("environments".into()))
        {
            for (i, env) in envs.iter().enumerate() {
                if let serde_yaml_ng::Value::Mapping(em) = env {
                    yaml_check_keys(
                        em,
                        &[
                            "name",
                            "user",
                            "ssh_key",
                            "mode",
                            "ssh_port",
                            "ssh_options",
                            "wallix",
                            "wallix_group",
                            "jump",
                            "servers",
                            "probe_filesystems",
                            "tunnels",
                            "tags",
                        ],
                        file,
                        &format!("{ctx}.environments[{i}]"),
                        warnings,
                    );
                    if let Some(serde_yaml_ng::Value::Sequence(svs)) =
                        em.get(serde_yaml_ng::Value::String("servers".into()))
                    {
                        for (j, s) in svs.iter().enumerate() {
                            yaml_validate_entry(
                                s,
                                file,
                                &format!("{ctx}.environments[{i}].servers[{j}]"),
                                warnings,
                            );
                        }
                    }
                }
            }
        }

        if let Some(serde_yaml_ng::Value::Sequence(svs)) =
            m.get(serde_yaml_ng::Value::String("servers".into()))
        {
            for (j, s) in svs.iter().enumerate() {
                yaml_validate_entry(s, file, &format!("{ctx}.servers[{j}]"), warnings);
            }
        }
    }
}

fn yaml_check_keys(
    m: &serde_yaml_ng::Mapping,
    known: &[&str],
    file: &str,
    ctx: &str,
    warnings: &mut Vec<ValidationWarning>,
) {
    for key in m.keys() {
        if let serde_yaml_ng::Value::String(k) = key
            && !known.contains(&k.as_str())
        {
            warnings.push(ValidationWarning {
                file: file.to_string(),
                context: ctx.to_string(),
                field: k.clone(),
            });
        }
    }
}
