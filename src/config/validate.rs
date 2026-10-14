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
    let value: YamlValue = match parse_yaml(content) {
        Ok(v) => v,
        Err(_) => return vec![], // l'erreur de parsing est déjà remontée par serde
    };

    let mut warnings = Vec::new();

    if let YamlValue::Mapping(root) = &value {
        yaml_check_keys(
            root,
            &["defaults", "groups", "includes", "_vars"],
            file_path,
            "root",
            &mut warnings,
        );

        if let Some(YamlValue::Mapping(m)) = root.get("defaults") {
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
                    "production",
                    "confirm_production",
                ],
                file_path,
                "defaults",
                &mut warnings,
            );
        }

        if let Some(YamlValue::Sequence(incs)) = root.get("includes") {
            for (i, inc) in incs.iter().enumerate() {
                if let YamlValue::Mapping(m) = inc {
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

        if let Some(YamlValue::Sequence(groups)) = root.get("groups") {
            for (i, g) in groups.iter().enumerate() {
                yaml_validate_entry(g, file_path, &format!("groups[{i}]"), &mut warnings);
            }
        }
    }

    warnings
}

fn yaml_validate_entry(
    val: &YamlValue,
    file: &str,
    ctx: &str,
    warnings: &mut Vec<ValidationWarning>,
) {
    let YamlValue::Mapping(m) = val else {
        return;
    };
    let has_host = m.contains_key("host");
    let has_envs = m.contains_key("environments");

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
                "production",
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
                "production",
            ],
            file,
            ctx,
            warnings,
        );

        if let Some(YamlValue::Sequence(envs)) = m.get("environments") {
            for (i, env) in envs.iter().enumerate() {
                if let YamlValue::Mapping(em) = env {
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
                            "production",
                        ],
                        file,
                        &format!("{ctx}.environments[{i}]"),
                        warnings,
                    );
                    if let Some(YamlValue::Sequence(svs)) = em.get("servers") {
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

        if let Some(YamlValue::Sequence(svs)) = m.get("servers") {
            for (j, s) in svs.iter().enumerate() {
                yaml_validate_entry(s, file, &format!("{ctx}.servers[{j}]"), warnings);
            }
        }
    }
}

fn yaml_check_keys(
    m: &YamlMapping,
    known: &[&str],
    file: &str,
    ctx: &str,
    warnings: &mut Vec<ValidationWarning>,
) {
    for key in m.keys() {
        if let YamlValue::String(k) = key
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

// ─── Arbre YAML générique ────────────────────────────────────────────────────

/// Arbre YAML minimal pour la validation des clés (ordre source conservé).
///
/// `serde_saphyr` désérialise directement vers des types Rust, sans arbre
/// intermédiaire : ce type en tient lieu, construit via `deserialize_any`.
#[derive(Debug)]
pub(crate) enum YamlValue {
    String(String),
    Mapping(YamlMapping),
    Sequence(Vec<YamlValue>),
    /// Nombre, booléen ou null : sans intérêt pour la validation des clés.
    Other,
}

/// Mapping YAML : paires clé/valeur dans l'ordre du fichier.
#[derive(Debug, Default)]
pub(crate) struct YamlMapping(Vec<(YamlValue, YamlValue)>);

impl YamlMapping {
    /// Valeur associée à la clé chaîne `key`.
    pub(crate) fn get(&self, key: &str) -> Option<&YamlValue> {
        self.0
            .iter()
            .find(|(k, _)| matches!(k, YamlValue::String(s) if s == key))
            .map(|(_, v)| v)
    }

    pub(crate) fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &YamlValue> {
        self.0.iter().map(|(k, _)| k)
    }
}

impl<'de> serde::Deserialize<'de> for YamlValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct YamlVisitor;

        impl<'de> serde::de::Visitor<'de> for YamlVisitor {
            type Value = YamlValue;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any YAML value")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<YamlValue, E> {
                Ok(YamlValue::String(v.to_string()))
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<YamlValue, E> {
                Ok(YamlValue::String(v))
            }

            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<YamlValue, E> {
                Ok(YamlValue::Other)
            }

            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<YamlValue, E> {
                Ok(YamlValue::Other)
            }

            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<YamlValue, E> {
                Ok(YamlValue::Other)
            }

            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<YamlValue, E> {
                Ok(YamlValue::Other)
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<YamlValue, E> {
                Ok(YamlValue::Other)
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<YamlValue, E> {
                Ok(YamlValue::Other)
            }

            fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<YamlValue, D::Error> {
                YamlValue::deserialize(d)
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<YamlValue, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(YamlValue::Sequence(items))
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<YamlValue, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(YamlValue::Mapping(YamlMapping(entries)))
            }
        }

        deserializer.deserialize_any(YamlVisitor)
    }
}
