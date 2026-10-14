use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;

// 2 niveaux d'includes imbriqués (fichier principal → include → include de
// l'include), pas plus : au-delà, les chaînes de configuration deviennent
// difficiles à suivre.
const MAX_INCLUDE_DEPTH: u32 = 2;
const MAX_FILE_SIZE_BYTES: u64 = 5 * 1024 * 1024; // 5 MiB
const HTTP_TIMEOUT_SECS: u64 = 10;

/// Désérialise un document YAML de configuration.
///
/// L'heuristique de ratio alias/ancres de `serde_saphyr` est désactivée : les
/// inventaires réutilisent volontairement quelques ancres des centaines de fois
/// (`probe_filesystems: *MNT`). Les limites absolues du budget (nombre d'alias,
/// événements rejoués, octets retenus) restent actives contre l'explosion
/// d'alias (« billion laughs »).
pub(crate) fn parse_yaml<'de, T: Deserialize<'de>>(
    content: &'de str,
) -> Result<T, serde_saphyr::Error> {
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            enforce_alias_anchor_ratio: false,
        },
    };
    serde_saphyr::from_str_with_options(content, options)
}

#[path = "config/types.rs"]
mod types;
pub use types::*;

#[path = "config/resolve.rs"]
mod resolve;
#[cfg(test)]
pub(crate) use resolve::merge_bastion;

#[path = "config/interpolate.rs"]
mod interpolate;
pub(crate) use interpolate::{extend_filesystems, extend_tags, merge_default_structs};
pub use interpolate::{interpolate, undefined_vars};

#[path = "config/validate.rs"]
mod validate;
pub(crate) use validate::fetch_url;
pub use validate::validate_yaml;

#[path = "config/git_status.rs"]
mod git_status;
pub use git_status::git_outdated_warnings;

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
