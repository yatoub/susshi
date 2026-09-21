use super::tests_helpers::make_namespace_config;
use super::*;

fn server_from_yaml(defaults: &str) -> ResolvedServer {
    let yaml = format!(
        r#"
defaults:
{defaults}
groups:
  - name: G
    environments:
      - name: prod
        servers:
          - name: srv
            host: "203.0.113.4"
      - name: dev
        servers:
          - name: dev-srv
            host: "203.0.113.5"
"#
    );
    let config: Config = serde_yaml_ng::from_str(&yaml).unwrap();
    let servers = config.resolve().unwrap();
    servers.into_iter().find(|s| s.name == "srv").unwrap()
}

fn dev_server(defaults: &str) -> ResolvedServer {
    let mut s = server_from_yaml(defaults);
    s.production = false;
    s
}

fn make_app() -> App {
    App::new(
        make_namespace_config(),
        vec![],
        std::path::PathBuf::new(),
        vec![],
    )
    .unwrap()
}

#[test]
fn confirmation_not_needed_when_not_production() {
    let app = make_app();
    let server = dev_server("  confirm_production: true");
    assert!(!app.needs_production_confirmation(&server));
}

#[test]
fn confirmation_not_needed_when_opt_in_disabled() {
    let app = make_app();
    let server = server_from_yaml("  keep_open: false");
    assert!(server.production);
    assert!(!app.needs_production_confirmation(&server));
}

#[test]
fn confirmation_needed_when_production_and_opted_in() {
    let app = make_app();
    let server = server_from_yaml("  confirm_production: true");
    assert!(app.needs_production_confirmation(&server));
}

#[test]
fn open_production_confirm_sets_mode() {
    let mut app = make_app();
    let server = server_from_yaml("  confirm_production: true");
    app.open_production_confirm(server, ConnectionMode::Direct, true);
    assert!(matches!(
        &app.app_mode,
        AppMode::ConfirmProduction { verbose: true, .. }
    ));
}

#[test]
fn accept_production_confirm_returns_connection_and_resets_mode() {
    let mut app = make_app();
    let server = server_from_yaml("  confirm_production: true");
    app.open_production_confirm(server, ConnectionMode::Direct, true);

    let (srv, mode, verbose) = app.accept_production_confirm().expect("pending confirm");
    assert_eq!(srv.name, "srv");
    assert_eq!(mode, ConnectionMode::Direct);
    assert!(verbose);
    assert_eq!(app.app_mode, AppMode::Normal);
}

#[test]
fn cancel_production_confirm_resets_mode() {
    let mut app = make_app();
    let server = server_from_yaml("  confirm_production: true");
    app.open_production_confirm(server, ConnectionMode::Direct, false);
    app.cancel_production_confirm();
    assert_eq!(app.app_mode, AppMode::Normal);
}

#[test]
fn accept_production_confirm_without_pending_returns_none() {
    let mut app = make_app();
    assert!(app.accept_production_confirm().is_none());
    assert_eq!(app.app_mode, AppMode::Normal);
}
