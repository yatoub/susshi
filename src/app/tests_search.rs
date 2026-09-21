use super::*;

fn make_tagged_config() -> Config {
    use crate::config::{Group, Server};
    Config {
        defaults: None,
        includes: vec![],
        groups: vec![ConfigEntry::Group(Group {
            name: "G".to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            probe_filesystems: None,
            environments: None,
            tunnels: None,
            tags: None,
            production: None,
            servers: Some(vec![
                Server {
                    name: "prod-web".to_string(),
                    host: "203.0.113.1".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: Some(vec!["prod".to_string(), "web".to_string()]),
                    ..Default::default()
                },
                Server {
                    name: "staging-db".to_string(),
                    host: "203.0.113.2".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: Some(vec!["staging".to_string(), "db".to_string()]),
                    ..Default::default()
                },
            ]),
        })],
        vars: Default::default(),
    }
}

#[test]
fn test_parse_tokens_text_only() {
    let (text, tags) = parse_search_tokens("web DB");
    assert_eq!(text, vec!["web", "db"]);
    assert!(tags.is_empty());
}

#[test]
fn test_parse_tokens_tags_only() {
    let (text, tags) = parse_search_tokens("#prod #eu");
    assert!(text.is_empty());
    assert_eq!(tags, vec!["prod", "eu"]);
}

#[test]
fn test_parse_tokens_mixed() {
    let (text, tags) = parse_search_tokens("web #prod DB");
    assert_eq!(text, vec!["web", "db"]);
    assert_eq!(tags, vec!["prod"]);
}

#[test]
fn test_parse_tokens_empty_hash() {
    let (text, tags) = parse_search_tokens("# word");
    assert_eq!(text, vec!["word"]);
    assert!(tags.is_empty());
}

#[test]
fn test_tag_filter_matches() {
    let config = make_tagged_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    app.search_query = "#prod".to_string();
    app.invalidate_cache();
    let items = app.get_visible_items();

    let has_prod = items.iter().any(|i| match i {
        ConfigItem::Server(s) => s.name == "prod-web",
        _ => false,
    });
    let has_staging = items.iter().any(|i| match i {
        ConfigItem::Server(s) => s.name == "staging-db",
        _ => false,
    });
    assert!(has_prod, "prod-web doit etre visible avec #prod");
    assert!(
        !has_staging,
        "staging-db ne doit pas etre visible avec #prod"
    );
}

#[test]
fn test_tag_filter_and_text() {
    let config = make_tagged_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    app.search_query = "#prod web".to_string();
    app.invalidate_cache();
    let items = app.get_visible_items();

    let has_prod_web = items.iter().any(|i| match i {
        ConfigItem::Server(s) => s.name == "prod-web",
        _ => false,
    });
    assert!(has_prod_web, "prod-web correspond a #prod web");
}

#[test]
fn test_tag_filter_no_match() {
    let config = make_tagged_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    app.search_query = "#inexistant".to_string();
    app.invalidate_cache();
    let items = app.get_visible_items();

    let has_server = items.iter().any(|i| matches!(i, ConfigItem::Server(_)));
    assert!(
        !has_server,
        "Aucun serveur ne doit correspondre a #inexistant"
    );
}
