use super::*;

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yaml_ng::from_str(&content)?;
        config.sort();
        Ok(config)
    }

    /// Alias pratique pour les tests et cas où seul le `Config` est nécessaire.
    /// Utiliser `load_merged` en production pour obtenir les avertissements de validation.
    #[cfg(test)]
    pub fn load_simple<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        Self::load(path)
    }

    pub fn sort(&mut self) {
        // Sort top-level entries (Groups, Servers, Namespaces)
        self.groups.sort_by(|a, b| {
            let name_a = match a {
                ConfigEntry::Group(g) => g.name.as_str(),
                ConfigEntry::Server(s) => s.name.as_str(),
                ConfigEntry::Namespace(ns) => ns.label.as_str(),
            };
            let name_b = match b {
                ConfigEntry::Group(g) => g.name.as_str(),
                ConfigEntry::Server(s) => s.name.as_str(),
                ConfigEntry::Namespace(ns) => ns.label.as_str(),
            };
            name_a.cmp(name_b)
        });

        // Sort children
        for entry in &mut self.groups {
            match entry {
                ConfigEntry::Group(group) => sort_group(group),
                ConfigEntry::Namespace(ns) => {
                    ns.entries.sort_by(|a, b| {
                        let name_a = match a {
                            ConfigEntry::Group(g) => g.name.as_str(),
                            ConfigEntry::Server(s) => s.name.as_str(),
                            ConfigEntry::Namespace(n) => n.label.as_str(),
                        };
                        let name_b = match b {
                            ConfigEntry::Group(g) => g.name.as_str(),
                            ConfigEntry::Server(s) => s.name.as_str(),
                            ConfigEntry::Namespace(n) => n.label.as_str(),
                        };
                        name_a.cmp(name_b)
                    });
                    for sub_entry in &mut ns.entries {
                        if let ConfigEntry::Group(group) = sub_entry {
                            sort_group(group);
                        }
                    }
                }
                ConfigEntry::Server(_) => {}
            }
        }
    }

    pub fn resolve(&self) -> Result<Vec<ResolvedServer>, ConfigError> {
        let mut resolved = Vec::new();

        let d = self.defaults.clone().unwrap_or_default();
        let use_sys_cfg = d.use_system_ssh_config.unwrap_or(false);

        for entry in &self.groups {
            match entry {
                ConfigEntry::Namespace(ns) => {
                    // Les defaults du fichier principal servent de base ; les defaults
                    // locaux du namespace (sous-fichier) les surchargent champ par champ.
                    let ns_local = ns.defaults.clone().unwrap_or_default();
                    let ns_d = merge_default_structs(&d, &ns_local);
                    let ns_use_sys_cfg = ns_d.use_system_ssh_config.unwrap_or(use_sys_cfg);
                    resolve_entries(
                        &ns.entries,
                        &ns_d,
                        ns_use_sys_cfg,
                        &ns.label,
                        &mut resolved,
                        &ns.vars,
                    )?;
                }
                _ => {
                    resolve_entries(
                        std::slice::from_ref(entry),
                        &d,
                        use_sys_cfg,
                        "",
                        &mut resolved,
                        &self.vars,
                    )?;
                }
            }
        }

        Ok(resolved)
    }

    /// Charge le fichier principal et résout tous les `includes` récursivement.
    ///
    /// Retourne le `Config` fusionné, les avertissements d'includes non-bloquants
    /// et les avertissements de validation YAML (champs inconnus).
    ///
    /// `loading_stack` sert à détecter les cycles ; passer `&mut HashSet::new()`
    /// à l'appel de premier niveau.
    pub fn load_merged<P: AsRef<Path>>(
        path: P,
        loading_stack: &mut HashSet<PathBuf>,
        depth: u32,
    ) -> Result<(Self, Vec<IncludeWarning>, Vec<ValidationWarning>), ConfigError> {
        let path = path.as_ref();
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        if depth > MAX_INCLUDE_DEPTH {
            return Err(ConfigError::IncludeDepthExceeded {
                path: canonical.display().to_string(),
                limit: MAX_INCLUDE_DEPTH,
            });
        }

        let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if file_size > MAX_FILE_SIZE_BYTES {
            return Err(ConfigError::FileTooLarge {
                path: canonical.display().to_string(),
                limit: MAX_FILE_SIZE_BYTES,
            });
        }

        // Lecture du contenu pour la validation ET le parsing.
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = serde_yaml_ng::from_str(&content)?;
        config.sort();

        let mut inc_warnings: Vec<IncludeWarning> = Vec::new();
        let mut val_warnings: Vec<ValidationWarning> =
            validate_yaml(&content, &canonical.display().to_string());

        if config.includes.is_empty() {
            return Ok((config, inc_warnings, val_warnings));
        }

        loading_stack.insert(canonical.clone());
        let parent_dir = canonical
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let includes = std::mem::take(&mut config.includes);
        let main_defaults = config.defaults.clone().unwrap_or_default();

        for entry in includes {
            // ── Include URL HTTPS ────────────────────────────────────────────
            if entry.path.starts_with("https://") || entry.path.starts_with("http://") {
                let url = entry.path.clone();
                let content = match fetch_url(&url) {
                    Ok(c) => c,
                    Err(e) => {
                        inc_warnings.push(IncludeWarning::LoadError {
                            label: entry.label.clone(),
                            path: url.clone(),
                            error: e,
                        });
                        continue;
                    }
                };
                let mut sub_config: Config = match serde_yaml_ng::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        inc_warnings.push(IncludeWarning::LoadError {
                            label: entry.label.clone(),
                            path: url.clone(),
                            error: e.to_string(),
                        });
                        continue;
                    }
                };
                sub_config.sort();
                let sub_val = validate_yaml(&content, &url);
                val_warnings.extend(sub_val);
                if entry.merge_defaults {
                    let sub_d = sub_config.defaults.unwrap_or_default();
                    sub_config.defaults = Some(merge_default_structs(&main_defaults, &sub_d));
                }
                let mut direct_entries = Vec::new();
                let mut nested_namespaces: Vec<NamespaceEntry> = Vec::new();
                for sub_entry in sub_config.groups {
                    match sub_entry {
                        ConfigEntry::Namespace(ns) => nested_namespaces.push(ns),
                        other => direct_entries.push(other),
                    }
                }
                config.groups.push(ConfigEntry::Namespace(NamespaceEntry {
                    label: entry.label.clone(),
                    source_path: url.clone(),
                    defaults: sub_config.defaults,
                    entries: direct_entries,
                    vars: sub_config.vars,
                }));
                for nested in nested_namespaces {
                    config.groups.push(ConfigEntry::Namespace(nested));
                }
                continue;
            }

            // ── Include fichier local ────────────────────────────────────────
            // Résolution du chemin (tilde + relatif)
            let expanded = shellexpand::tilde(&entry.path).into_owned();
            let raw = std::path::Path::new(&expanded);
            let sub_path = if raw.is_absolute() {
                raw.to_path_buf()
            } else {
                parent_dir.join(raw)
            };

            // Forme canonique pour la détection de cycle
            let sub_canonical = match std::fs::canonicalize(&sub_path) {
                Ok(p) => p,
                Err(e) => {
                    inc_warnings.push(IncludeWarning::LoadError {
                        label: entry.label.clone(),
                        path: sub_path.display().to_string(),
                        error: e.to_string(),
                    });
                    continue;
                }
            };

            if loading_stack.contains(&sub_canonical) {
                inc_warnings.push(IncludeWarning::Circular {
                    label: entry.label.clone(),
                    path: sub_canonical.display().to_string(),
                });
                continue;
            }

            // Chargement récursif du sous-fichier
            let (mut sub_config, mut sub_inc, sub_val) =
                match Self::load_merged(&sub_path, loading_stack, depth + 1) {
                    Ok(r) => r,
                    // Profondeur excessive : erreur dure, pas avertissement silencieux.
                    Err(e @ ConfigError::IncludeDepthExceeded { .. }) => return Err(e),
                    Err(e) => {
                        inc_warnings.push(IncludeWarning::LoadError {
                            label: entry.label.clone(),
                            path: sub_path.display().to_string(),
                            error: e.to_string(),
                        });
                        continue;
                    }
                };
            inc_warnings.append(&mut sub_inc);
            val_warnings.extend(sub_val);

            // Fusion optionnelle des defaults du fichier principal
            if entry.merge_defaults {
                let sub_d = sub_config.defaults.unwrap_or_default();
                sub_config.defaults = Some(merge_default_structs(&main_defaults, &sub_d));
            }

            // Partition : entrées directes vs namespaces imbriqués (issus d'includes récursifs)
            let mut direct_entries = Vec::new();
            let mut nested_namespaces: Vec<NamespaceEntry> = Vec::new();
            for sub_entry in sub_config.groups {
                match sub_entry {
                    ConfigEntry::Namespace(ns) => nested_namespaces.push(ns),
                    other => direct_entries.push(other),
                }
            }

            // Namespace principal avec les entrées directes
            config.groups.push(ConfigEntry::Namespace(NamespaceEntry {
                label: entry.label.clone(),
                source_path: sub_canonical.display().to_string(),
                defaults: sub_config.defaults,
                entries: direct_entries,
                vars: sub_config.vars.clone(),
            }));

            // Namespaces imbriqués aplatis avec label préfixé "parent / enfant"
            for nested in nested_namespaces {
                config.groups.push(ConfigEntry::Namespace(NamespaceEntry {
                    label: format!("{} / {}", entry.label, nested.label),
                    source_path: nested.source_path,
                    defaults: nested.defaults,
                    entries: nested.entries,
                    vars: nested.vars,
                }));
            }
        }

        loading_stack.remove(&canonical);
        config.sort();

        Ok((config, inc_warnings, val_warnings))
    }
}

/// Résout un slice d'entrées de configuration avec les defaults et le namespace donnés.
fn resolve_entries(
    entries: &[ConfigEntry],
    d: &Defaults,
    use_sys_cfg: bool,
    namespace: &str,
    resolved: &mut Vec<ResolvedServer>,
    vars: &HashMap<String, String>,
) -> Result<(), ConfigError> {
    for entry in entries {
        match entry {
            ConfigEntry::Group(group) => {
                // Merge defaults -> Group
                let g_user = group.user.as_deref().or(d.user.as_deref());
                let g_key = group.ssh_key.as_deref().or(d.ssh_key.as_deref());
                let g_mode = group.mode.or(d.mode);
                let g_port = group.ssh_port.or(d.ssh_port);
                let g_opts = if let Some(opts) = &group.ssh_options {
                    Some(opts.clone())
                } else {
                    d.ssh_options.clone()
                };
                let g_fs = extend_filesystems(
                    d.probe_filesystems.as_ref(),
                    group.probe_filesystems.as_ref(),
                );

                let g_bastion = merge_bastion(&d.wallix, &group.wallix);
                let g_jump = merge_jump(&d.jump, &group.jump);
                let g_tunnels = replace_tunnels(&d.tunnels, &group.tunnels);
                let g_tags = extend_tags(d.tags.as_ref(), group.tags.as_ref());

                if let Some(envs) = &group.environments {
                    for env in envs {
                        // Merge Group -> Env
                        let e_user = env.user.as_deref().or(g_user);
                        let e_key = env.ssh_key.as_deref().or(g_key);
                        let e_mode = env.mode.or(g_mode);
                        let e_port = env.ssh_port.or(g_port);
                        let e_opts = if let Some(opts) = &env.ssh_options {
                            Some(opts.clone())
                        } else {
                            g_opts.clone()
                        };
                        let e_fs = extend_filesystems(
                            g_fs.as_ref().map(|v| v as &Vec<String>),
                            env.probe_filesystems.as_ref(),
                        );

                        let e_bastion = merge_bastion(&g_bastion, &env.wallix);
                        let e_jump = merge_jump(&g_jump, &env.jump);
                        let e_tunnels = replace_tunnels(&g_tunnels, &env.tunnels);
                        let e_tags = extend_tags(Some(&g_tags), env.tags.as_ref());

                        let env_def = ServerDefaults {
                            user: e_user,
                            key: e_key,
                            cert: d.ssh_cert.as_deref(),
                            agent_sock: d.ssh_agent_sock.as_deref(),
                            mode: e_mode,
                            port: e_port,
                            opts: e_opts.as_ref(),
                            bastion: &e_bastion,
                            jump: &e_jump,
                            use_system_ssh_config: use_sys_cfg,
                            fs: e_fs.clone(),
                            tunnels: e_tunnels.as_ref(),
                            namespace,
                            vars,
                            control_master: d.control_master.unwrap_or(false),
                            agent_forwarding: d.agent_forwarding.unwrap_or(false),
                            control_path: d
                                .control_path
                                .as_deref()
                                .unwrap_or("~/.ssh/ctl/%h_%p_%r"),
                            control_persist: d.control_persist.as_deref().unwrap_or("10m"),
                            pre_connect_hook: d.pre_connect_hook.as_deref(),
                            post_disconnect_hook: d.post_disconnect_hook.as_deref(),
                            hook_timeout_secs: d.hook_timeout_secs.unwrap_or(5),
                            tags: e_tags,
                        };
                        for server in &env.servers {
                            let r = resolve_server(server, &group.name, &env.name, &env_def)?;
                            resolved.push(r);
                        }
                    }
                }

                if let Some(servers) = &group.servers {
                    let grp_def = ServerDefaults {
                        user: g_user,
                        key: g_key,
                        cert: d.ssh_cert.as_deref(),
                        agent_sock: d.ssh_agent_sock.as_deref(),
                        mode: g_mode,
                        port: g_port,
                        opts: g_opts.as_ref(),
                        bastion: &g_bastion,
                        jump: &g_jump,
                        use_system_ssh_config: use_sys_cfg,
                        fs: g_fs.clone(),
                        tunnels: g_tunnels.as_ref(),
                        namespace,
                        vars,
                        control_master: d.control_master.unwrap_or(false),
                        agent_forwarding: d.agent_forwarding.unwrap_or(false),
                        control_path: d.control_path.as_deref().unwrap_or("~/.ssh/ctl/%h_%p_%r"),
                        control_persist: d.control_persist.as_deref().unwrap_or("10m"),
                        pre_connect_hook: d.pre_connect_hook.as_deref(),
                        post_disconnect_hook: d.post_disconnect_hook.as_deref(),
                        hook_timeout_secs: d.hook_timeout_secs.unwrap_or(5),
                        tags: g_tags.clone(),
                    };
                    for server in servers {
                        let r = resolve_server(server, &group.name, "", &grp_def)?;
                        resolved.push(r);
                    }
                }
            }
            ConfigEntry::Server(server) => {
                // Top-level server (root ou dans un namespace)
                let top_def = ServerDefaults {
                    user: d.user.as_deref(),
                    key: d.ssh_key.as_deref(),
                    cert: d.ssh_cert.as_deref(),
                    agent_sock: d.ssh_agent_sock.as_deref(),
                    mode: d.mode,
                    port: d.ssh_port,
                    opts: d.ssh_options.as_ref(),
                    bastion: &d.wallix,
                    jump: &d.jump,
                    use_system_ssh_config: use_sys_cfg,
                    fs: d.probe_filesystems.clone(),
                    tunnels: d.tunnels.as_ref(),
                    namespace,
                    vars,
                    control_master: d.control_master.unwrap_or(false),
                    agent_forwarding: d.agent_forwarding.unwrap_or(false),
                    control_path: d.control_path.as_deref().unwrap_or("~/.ssh/ctl/%h_%p_%r"),
                    control_persist: d.control_persist.as_deref().unwrap_or("10m"),
                    pre_connect_hook: d.pre_connect_hook.as_deref(),
                    post_disconnect_hook: d.post_disconnect_hook.as_deref(),
                    hook_timeout_secs: d.hook_timeout_secs.unwrap_or(5),
                    tags: extend_tags(None, d.tags.as_ref()),
                };
                let r = resolve_server(server, "", "", &top_def)?;
                resolved.push(r);
            }
            // Les namespaces imbriqués dans ns.entries ne sont jamais générés
            // après aplatissement dans load_merged — ce bras ne devrait pas être atteint.
            ConfigEntry::Namespace(_) => {}
        }
    }
    Ok(())
}

pub(crate) fn merge_bastion(
    parent: &Option<BastionConfig>,
    child: &Option<BastionConfig>,
) -> Option<BastionConfig> {
    match (parent, child) {
        (None, None) => None,
        (Some(p), None) => Some(p.clone()),
        (None, Some(c)) => Some(c.clone()),
        (Some(p), Some(c)) => Some(BastionConfig {
            host: c.host.clone().or(p.host.clone()),
            user: c.user.clone().or(p.user.clone()),
            group: c.group.clone().or(p.group.clone()),
            template: c.template.clone().or(p.template.clone()),
            account: c.account.clone().or(p.account.clone()),
            protocol: c.protocol.clone().or(p.protocol.clone()),
            auto_select: c.auto_select.or(p.auto_select),
            fail_if_menu_match_error: c.fail_if_menu_match_error.or(p.fail_if_menu_match_error),
            selection_timeout_secs: c.selection_timeout_secs.or(p.selection_timeout_secs),
            direct: c.direct.or(p.direct),
            authorization: c.authorization.clone().or(p.authorization.clone()),
            header_columns: c.header_columns.clone().or(p.header_columns.clone()),
        }),
    }
}

/// Le niveau enfant remplace entièrement le niveau parent (pas de fusion champ par champ),
/// ce qui permet de définir une chaîne de sauts complète à chaque niveau.
fn merge_jump(
    parent: &Option<Vec<JumpConfig>>,
    child: &Option<Vec<JumpConfig>>,
) -> Option<Vec<JumpConfig>> {
    child.clone().or_else(|| parent.clone())
}

/// Même sémantique que `merge_jump` : le niveau enfant remplace la liste parente en entier.
/// Contrairement à `extend_filesystems`, les tunnels ne s'accumulent pas.
fn replace_tunnels(
    parent: &Option<Vec<TunnelConfig>>,
    child: &Option<Vec<TunnelConfig>>,
) -> Option<Vec<TunnelConfig>> {
    child.clone().or_else(|| parent.clone())
}

/// Trie les environnements et serveurs d'un groupe.
fn sort_group(group: &mut Group) {
    if let Some(envs) = &mut group.environments {
        envs.sort_by(|a, b| a.name.cmp(&b.name));
        for env in envs.iter_mut() {
            env.servers.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }
    if let Some(servers) = &mut group.servers {
        servers.sort_by(|a, b| a.name.cmp(&b.name));
    }
}

/// Defaults effectifs à passer à `resolve_server`, après cascade group→env.
struct ServerDefaults<'a> {
    user: Option<&'a str>,
    key: Option<&'a str>,
    cert: Option<&'a str>,
    agent_sock: Option<&'a str>,
    mode: Option<ConnectionMode>,
    port: Option<u16>,
    opts: Option<&'a Vec<String>>,
    bastion: &'a Option<BastionConfig>,
    jump: &'a Option<Vec<JumpConfig>>,
    use_system_ssh_config: bool,
    fs: Option<Vec<String>>,
    tunnels: Option<&'a Vec<TunnelConfig>>,
    namespace: &'a str,
    vars: &'a HashMap<String, String>,
    control_master: bool,
    agent_forwarding: bool,
    control_path: &'a str,
    control_persist: &'a str,
    pre_connect_hook: Option<&'a str>,
    post_disconnect_hook: Option<&'a str>,
    hook_timeout_secs: u64,
    /// Tags accumulés depuis defaults → group → env (union sans doublon).
    tags: Vec<String>,
}

fn resolve_server(
    s: &Server,
    group: &str,
    env: &str,
    def: &ServerDefaults<'_>,
) -> Result<ResolvedServer, ConfigError> {
    let def_user = def.user;
    let def_key = def.key;
    let def_cert = def.cert;
    let def_agent_sock = def.agent_sock;
    let def_mode = def.mode;
    let def_port = def.port;
    let def_opts = def.opts;
    let def_bastion = def.bastion;
    let def_jump = def.jump;
    let use_system_ssh_config = def.use_system_ssh_config;
    let def_tunnels = def.tunnels;
    let namespace = def.namespace;
    let vars = def.vars;
    let def_control_master = def.control_master;
    let def_agent_forwarding = def.agent_forwarding;
    let def_control_path = def.control_path;
    let def_control_persist = def.control_persist;
    let def_pre_connect_hook = def.pre_connect_hook;
    let def_post_disconnect_hook = def.post_disconnect_hook;
    let def_hook_timeout_secs = def.hook_timeout_secs;
    let user = interpolate(s.user.as_deref().or(def_user).unwrap_or("root"), vars);
    let port = s.ssh_port.or(def_port).unwrap_or(22);
    let key = interpolate(
        s.ssh_key.as_deref().or(def_key).unwrap_or("~/.ssh/id_rsa"),
        vars,
    );
    let cert = s
        .ssh_cert
        .as_deref()
        .or(def_cert)
        .map(|c| shellexpand::tilde(c).into_owned())
        .unwrap_or_default();
    let agent_sock = s
        .ssh_agent_sock
        .as_deref()
        .or(def_agent_sock)
        .map(|c| shellexpand::tilde(c).into_owned())
        .unwrap_or_default();

    let opts = if let Some(o) = &s.ssh_options {
        o.clone()
    } else {
        def_opts.cloned().unwrap_or_default()
    };

    let probe_filesystems =
        extend_filesystems(def.fs.as_ref(), s.probe_filesystems.as_ref()).unwrap_or_default();

    let tunnels = s
        .tunnels
        .as_ref()
        .or(def_tunnels)
        .cloned()
        .unwrap_or_default();

    let final_bastion = merge_bastion(def_bastion, &s.wallix);
    let final_jump = merge_jump(def_jump, &s.jump);

    let mode = s.mode.or(def_mode).unwrap_or(ConnectionMode::Direct);

    let bastion_template = final_bastion
        .as_ref()
        .and_then(|b| b.template.clone())
        .unwrap_or_else(|| "{target_user}@%n:SSH:{bastion_user}".to_string());

    let jump_host = final_jump.as_ref().map(|jumps| {
        jumps
            .iter()
            .map(|j| {
                let h = j.host.as_deref().unwrap_or("");
                let u = j.user.as_deref().unwrap_or(&user);
                format!("{u}@{h}")
            })
            .collect::<Vec<_>>()
            .join(",")
    });

    // Priorité déterministe pour le groupe Wallix:
    // 1) server.wallix.group
    // 2) server.wallix_group (legacy)
    // 3) héritage déjà fusionné via final_bastion.group (env/group/defaults)
    let resolved_wallix_group = s
        .wallix
        .as_ref()
        .and_then(|b| b.group.as_deref())
        .or(s.wallix_group.as_deref())
        .or(final_bastion.as_ref().and_then(|b| b.group.as_deref()))
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .map(ToOwned::to_owned);

    Ok(ResolvedServer {
        namespace: namespace.to_string(),
        group_name: group.to_string(),
        env_name: env.to_string(),
        name: interpolate(&s.name, vars),
        host: interpolate(&s.host, vars),
        user,
        port,
        ssh_key: key,
        ssh_cert: cert,
        ssh_agent_sock: agent_sock,
        ssh_options: opts,
        default_mode: mode,
        jump_host,
        bastion_host: final_bastion.as_ref().and_then(|b| b.host.clone()),
        bastion_user: final_bastion.as_ref().and_then(|b| b.user.clone()),
        bastion_template,
        use_system_ssh_config,
        probe_filesystems,
        tunnels,
        tags: extend_tags(Some(&def.tags), s.tags.as_ref()),
        control_master: def_control_master,
        agent_forwarding: def_agent_forwarding,
        control_path: if def_control_master {
            shellexpand::tilde(def_control_path).into_owned()
        } else {
            String::new()
        },
        control_persist: def_control_persist.to_string(),
        pre_connect_hook: s
            .pre_connect_hook
            .as_deref()
            .or(def_pre_connect_hook)
            .map(|h| shellexpand::tilde(h).into_owned()),
        post_disconnect_hook: s
            .post_disconnect_hook
            .as_deref()
            .or(def_post_disconnect_hook)
            .map(|h| shellexpand::tilde(h).into_owned()),
        hook_timeout_secs: def_hook_timeout_secs,
        notes: s.notes.clone().unwrap_or_default(),
        wallix_group: resolved_wallix_group,
        wallix_account: final_bastion
            .as_ref()
            .and_then(|b| b.account.clone())
            .unwrap_or_else(|| "default".to_string()),
        wallix_protocol: final_bastion
            .as_ref()
            .and_then(|b| b.protocol.clone())
            .unwrap_or_else(|| "SSH".to_string()),
        wallix_auto_select: final_bastion
            .as_ref()
            .and_then(|b| b.auto_select)
            .unwrap_or(true),
        wallix_fail_if_menu_match_error: final_bastion
            .as_ref()
            .and_then(|b| b.fail_if_menu_match_error)
            .unwrap_or(true),
        wallix_selection_timeout_secs: final_bastion
            .as_ref()
            .and_then(|b| b.selection_timeout_secs)
            .unwrap_or(8),
        wallix_direct: final_bastion
            .as_ref()
            .and_then(|b| b.direct)
            .unwrap_or(false),
        wallix_authorization: final_bastion.as_ref().and_then(|b| b.authorization.clone()),
        wallix_header_columns: final_bastion
            .as_ref()
            .and_then(|b| b.header_columns.clone())
            .unwrap_or_default(),
    })
}
