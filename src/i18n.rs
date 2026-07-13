//! Internationalisation (i18n) — textes de l'interface TUI.
//!
//! Langues supportées détectées depuis les variables d'environnement POSIX
//! (`LC_ALL` → `LC_MESSAGES` → `LANG`). Anglais par défaut.
//!
//! Les chaînes localisées sont définies dans `i18n/<lang>/susshi.ftl` et
//! embarquées dans le binaire à la compilation via `rust-embed`.
//!
//! ## Utilisation
//! ```rust,ignore
//! use crate::fl;
//! let title: String = fl!("error-title");
//! let msg: String = fl!("tunnel-started", label = "srv", port = "2222");
//! ```

use i18n_embed::{
    DesktopLanguageRequester,
    fluent::{FluentLanguageLoader, fluent_language_loader},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "i18n/"]
struct Localizations;

/// The static language loader, lazily initialised with the fallback locale
/// defined in `i18n.toml`. Language selection happens later in [`init`].
pub static LANGUAGE_LOADER: std::sync::LazyLock<FluentLanguageLoader> =
    std::sync::LazyLock::new(|| fluent_language_loader!());

/// Initialise le chargeur i18n depuis les variables d'environnement POSIX.
/// Doit être appelé une seule fois au démarrage, avant toute utilisation de [`fl!`].
///
/// En cas d'échec de sélection de langue (locale inconnue, ressources
/// manquantes, ...), on se rabat silencieusement sur la langue par défaut
/// définie dans `i18n.toml` plutôt que de faire planter l'application.
pub fn init() {
    let requested = DesktopLanguageRequester::requested_languages();
    if let Err(err) = i18n_embed::select(&*LANGUAGE_LOADER, &Localizations, &requested) {
        eprintln!("warning: failed to select i18n language, falling back to default: {err}");
    }
}

/// Macro de traduction. Retourne un [`String`] localisé.
///
/// ```rust,ignore
/// fl!("error-title")
/// fl!("tunnel-started", label = server_name, port = port_str)
/// fl!("tunnel-badge-active", n_run = (count as i64), n_cfg = (total as i64))
/// ```
#[macro_export]
macro_rules! fl {
    ($message_id:literal) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id)
    }};
    ($message_id:literal, $($key:ident = $value:expr),+ $(,)?) => {{
        i18n_embed_fl::fl!($crate::i18n::LANGUAGE_LOADER, $message_id, $($key = $value),+)
    }};
}
