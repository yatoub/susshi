use super::*;

/// Select a menu entry ID based on target and group matching.
///
/// This function implements the core selection algorithm:
/// 1. Filter entries by exact target match (user@account@host:protocol).
/// 2. Filter by exact group match (`wallix.group`, or legacy `wallix_group`).
/// 3. Return error if no match or multiple matches found.
/// 4. Return the ID if exactly one match.
pub fn select_id_by_target_and_group(
    entries: &[WallixMenuEntry],
    target: &str,
    group: &str,
) -> Result<String> {
    let target_matches: Vec<_> = entries
        .iter()
        .filter(|e| e.target.eq_ignore_ascii_case(target))
        .collect();

    if target_matches.is_empty() {
        return Err(anyhow!(
            "No menu entry found with target '{}'. Available targets: {}",
            target,
            entries
                .iter()
                .map(|e| e.target.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let group_matches: Vec<_> = target_matches
        .iter()
        .filter(|e| e.group.eq_ignore_ascii_case(group))
        .collect();

    match group_matches.len() {
        0 => Err(anyhow!(
            "No menu entry found for target '{}' with group '{}'. Available groups for this target: {}",
            target,
            group,
            target_matches
                .iter()
                .map(|e| e.group.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        1 => Ok(group_matches[0].id.clone()),
        n => Err(anyhow!(
            "Multiple menu entries ({}) found for target '{}' and group '{}'. Cannot auto-select.",
            n,
            target,
            group
        )),
    }
}

/// Select a Wallix menu entry directly from a resolved server configuration.
pub fn select_id_for_server(
    entries: &[WallixMenuEntry],
    server: &ResolvedServer,
) -> Result<String> {
    let targets = build_expected_targets(server);
    let groups = build_expected_groups(server)?;
    let configured_group = server
        .wallix_group
        .as_deref()
        .map(str::trim)
        .filter(|g| !g.is_empty());

    let mut had_target_match = false;
    let mut available_groups_for_matching_targets: Vec<String> = Vec::new();

    for target in &targets {
        let target_entries: Vec<&WallixMenuEntry> = entries
            .iter()
            .filter(|e| e.target.eq_ignore_ascii_case(target))
            .collect();
        if target_entries.is_empty() {
            continue;
        }

        had_target_match = true;

        for entry in &target_entries {
            if !available_groups_for_matching_targets.contains(&entry.group) {
                available_groups_for_matching_targets.push(entry.group.clone());
            }
        }

        // No group configured: auto-select only if exactly one entry matches the target.
        if configured_group.is_none() {
            match target_entries.len() {
                1 => return Ok(target_entries[0].id.clone()),
                _ => {
                    return Err(anyhow!(
                        "Multiple menu entries found for target '{}'. Configure wallix.group to auto-select. Available groups: {}",
                        target,
                        available_groups_for_matching_targets.join(", ")
                    ));
                }
            }
        }

        for group in &groups {
            let exact: Vec<_> = target_entries
                .iter()
                .filter(|entry| entry.group.eq_ignore_ascii_case(group))
                .collect();
            match exact.len() {
                1 => return Ok(exact[0].id.clone()),
                n if n > 1 => {
                    return Err(anyhow!(
                        "Multiple menu entries ({}) found for target '{}' and group '{}'. Cannot auto-select.",
                        n,
                        target,
                        group
                    ));
                }
                _ => {}
            }
        }

        let cg = configured_group.unwrap_or_default();
        let suffix_matches: Vec<_> = target_entries
            .iter()
            .filter(|entry| group_suffix_matches(&entry.group, cg))
            .collect();
        match suffix_matches.len() {
            1 => return Ok(suffix_matches[0].id.clone()),
            n if n > 1 => {
                return Err(anyhow!(
                    "Multiple menu entries ({}) found for target '{}' matching group suffix '{}'. Cannot auto-select.",
                    n,
                    target,
                    cg
                ));
            }
            _ => {}
        }
    }

    if had_target_match {
        return Err(anyhow!(
            "No menu entry found for matching targets with group '{}'. Available groups for these targets: {}",
            configured_group.unwrap_or("(none)"),
            available_groups_for_matching_targets.join(", ")
        ));
    }

    Err(anyhow!(
        "No menu entry found with target '{}'. Available targets: {}",
        targets
            .last()
            .cloned()
            .unwrap_or_else(|| build_expected_target(server)),
        entries
            .iter()
            .map(|e| e.target.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}
