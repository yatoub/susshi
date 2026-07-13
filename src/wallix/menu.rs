use super::*;

/// Parse Wallix menu output into structured entries.
///
/// The Wallix menu typically looks like:
/// ```text
///    ID │ Cible                              │ Autorisation
/// ───────┼────────────────────────────────────┼──────────────────────
///  1234  │ demo_user@default@APP-ALPHA-BD:SSH    │ APP-ALPHA_ops-admins
///  5678  │ demo_user@default@APP-ALPHA-BD:SSH    │ APP-ALPHA_dev-admins
/// ```
///
const DEFAULT_HEADER_COLUMNS: &[&str] = &["ID", "Cible", "Autorisation"];

/// This function extracts the ID, target (Cible), and group (Autorisation) columns.
///
/// `header_columns` overrides the default detection tokens used to skip header lines.
/// Pass an empty slice to use the defaults.
pub fn parse_wallix_menu(output: &str, header_columns: &[String]) -> Result<Vec<WallixMenuEntry>> {
    let cleaned = strip_ansi(output);
    let mut entries = Vec::new();

    let effective_headers: Vec<&str> = if header_columns.is_empty() {
        DEFAULT_HEADER_COLUMNS.to_vec()
    } else {
        header_columns.iter().map(String::as_str).collect()
    };

    for line in cleaned.lines() {
        let trimmed = line.trim();

        // Ignore headers, separators and empty lines.
        if trimmed.is_empty()
            || effective_headers.iter().any(|h| trimmed.contains(h))
            || trimmed
                .chars()
                .all(|c| matches!(c, '\u{2500}' | '\u{253C}' | '\u{2502}' | '-' | '+'))
        {
            continue;
        }

        let separator = if trimmed.contains('\u{2502}') {
            '\u{2502}'
        } else if trimmed.contains('|') {
            '|'
        } else {
            continue;
        };

        let mut columns = trimmed
            .split(separator)
            .map(str::trim)
            .filter(|column| !column.is_empty());
        let Some(id) = columns.next() else {
            continue;
        };
        let Some(target) = columns.next() else {
            continue;
        };
        let Some(group) = columns.next() else {
            continue;
        };

        if !id.is_empty()
            && id.chars().all(|character| character.is_ascii_digit())
            && !target.is_empty()
            && !group.is_empty()
        {
            entries.push(WallixMenuEntry {
                id: id.to_string(),
                target: target.to_string(),
                group: group.to_string(),
            });
        }
    }

    if entries.is_empty() {
        return Err(anyhow!("No valid menu entries found in Wallix output"));
    }

    Ok(entries)
}
