use super::tests_helpers::{make_namespace_config, make_namespace_env_config};
use super::*;
use crate::config::{ConfigEntry, Environment, Group, Server};

fn create_test_config() -> Config {
    Config {
        defaults: None,
        includes: vec![],
        groups: vec![ConfigEntry::Group(Group {
            name: "G1".to_string(),
            user: None,
            ssh_key: None,
            mode: None,
            ssh_port: None,
            ssh_options: None,
            wallix: None,
            wallix_group: None,
            jump: None,
            probe_filesystems: None,
            environments: Some(vec![Environment {
                name: "E1".to_string(),
                user: None,
                ssh_key: None,
                mode: None,
                ssh_port: None,
                ssh_options: None,
                wallix: None,
                wallix_group: None,
                jump: None,
                probe_filesystems: None,
                tunnels: None,
                tags: None,
                production: None,
                servers: vec![Server {
                    name: "S1".to_string(),
                    host: "198.51.100.1".to_string(),
                    user: None,
                    ssh_key: None,
                    ssh_port: None,
                    ssh_options: None,
                    mode: None,
                    wallix: None,
                    jump: None,
                    probe_filesystems: None,
                    tunnels: None,
                    tags: None,
                    production: None,
                    ..Default::default()
                }],
            }]),
            servers: Some(vec![Server {
                name: "S2".to_string(),
                host: "198.51.100.2".to_string(),
                user: None,
                ssh_key: None,
                ssh_port: None,
                ssh_options: None,
                mode: None,
                wallix: None,
                jump: None,
                probe_filesystems: None,
                tunnels: None,
                tags: None,
                production: None,
                ..Default::default()
            }]),
            tunnels: None,
            tags: None,
            production: None,
        })],
        vars: Default::default(),
    }
}

#[test]
fn test_initial_visibility() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    let items = app.get_visible_items();

    assert_eq!(items.len(), 1);
    match &items[0] {
        ConfigItem::Group(name, _ns) => assert_eq!(name, "G1"),
        _ => panic!("Expected Group G1"),
    }
}

#[test]
fn test_expansion() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    app.toggle_expansion();
    let items = app.get_visible_items();

    assert_eq!(items.len(), 3);

    match &items[1] {
        ConfigItem::Environment(g, e, _ns) => {
            assert_eq!(g, "G1");
            assert_eq!(e, "E1");
        }
        _ => panic!("Expected Environment E1"),
    }

    match &items[2] {
        ConfigItem::Server(s) => assert_eq!(s.name, "S2"),
        _ => panic!("Expected Server S2"),
    }
}

#[test]
fn test_collapse_all() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    app.toggle_expansion();
    app.selected_index = 1;
    app.items_dirty = true;
    app.toggle_expansion();

    assert!(!app.expanded_items.is_empty());

    app.collapse_all();

    assert!(app.expanded_items.is_empty());
    assert_eq!(app.selected_index, 0);
    let items = app.get_visible_items();
    assert_eq!(items.len(), 1);
}

#[test]
fn test_search_filtering() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    app.search_query = "S1".to_string();
    app.invalidate_cache();
    let items = app.get_visible_items();

    // En mode recherche, la liste est plate (serveurs uniquement, sans headers)
    assert!(!items.is_empty());
    assert!(
        items.iter().all(|i| matches!(i, ConfigItem::Server(_))),
        "En mode recherche, seuls des Server doivent apparaître"
    );

    let has_s1 = items.iter().any(|i| match i {
        ConfigItem::Server(s) => s.name == "S1",
        _ => false,
    });
    assert!(has_s1, "Should contain S1");

    let has_s2 = items.iter().any(|i| match i {
        ConfigItem::Server(s) => s.name == "S2",
        _ => false,
    });
    assert!(!has_s2, "Should NOT contain S2");
}

#[test]
fn test_namespace_visibility_collapsed() {
    let config = make_namespace_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.expanded_items.clear();
    app.invalidate_cache();
    let items = app.get_visible_items();

    assert_eq!(items.len(), 2);
    assert!(matches!(
        &items[0],
        ConfigItem::Group(name, ns) if name == "RootGroup" && ns.is_empty()
    ));
    assert!(matches!(&items[1], ConfigItem::Namespace(label) if label == "CES"));
}

#[test]
fn test_namespace_expansion() {
    let config = make_namespace_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.expanded_items.clear();
    app.invalidate_cache();

    app.select(1);
    app.toggle_expansion();

    let items = app.get_visible_items();

    assert_eq!(items.len(), 3);
    assert!(matches!(
        &items[2],
        ConfigItem::Group(name, ns) if name == "CES_Group" && ns == "CES"
    ));
}

#[test]
fn test_search_crosses_namespaces() {
    let config = make_namespace_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    app.search_query = "ces_srv".to_string();
    app.invalidate_cache();
    let items = app.get_visible_items();

    let has_ces = items.iter().any(|i| match i {
        ConfigItem::Server(s) => s.name == "ces_srv",
        _ => false,
    });
    assert!(has_ces, "Search should find ces_srv in namespace CES");

    let has_root = items.iter().any(|i| match i {
        ConfigItem::Server(s) => s.name == "root_srv",
        _ => false,
    });
    assert!(!has_root, "root_srv should be filtered out");
}

// ── namespace + favorites_only ────────────────────────────────────────────────

#[test]
fn namespace_hidden_when_no_favorites_inside() {
    let config = make_namespace_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.favorites.clear();
    app.favorites_only = true;
    app.items_dirty = true;

    let items = app.get_visible_items();
    // With no favorites, even the namespace header should be hidden.
    assert!(
        items.iter().all(|i| !matches!(i, ConfigItem::Namespace(_))),
        "Namespace without favorites should not appear"
    );
}

#[test]
fn namespace_shown_when_has_favorite() {
    let config = make_namespace_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.favorites.clear();

    // Favorite the server inside the CES namespace.
    let ces_server = app
        .resolved_servers
        .iter()
        .find(|s| s.namespace == "CES")
        .cloned()
        .unwrap();
    app.favorites.insert(App::server_key(&ces_server));
    app.favorites_only = true;
    app.items_dirty = true;

    let items = app.get_visible_items();
    let has_ns = items
        .iter()
        .any(|i| matches!(i, ConfigItem::Namespace(n) if n == "CES"));
    assert!(
        has_ns,
        "CES namespace should appear when it contains a favorite"
    );
}

#[test]
fn group_hidden_when_no_favorites_inside_namespace() {
    let config = make_namespace_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.favorites.clear();

    // Only favorite the root group server — nothing in the CES namespace.
    let root_server = app
        .resolved_servers
        .iter()
        .find(|s| s.namespace.is_empty())
        .cloned()
        .unwrap();
    app.favorites.insert(App::server_key(&root_server));
    app.favorites_only = true;

    // Expand the CES namespace so its group would show if it had favorites.
    app.expanded_items.insert("NS:CES".to_string());
    app.items_dirty = true;

    let items = app.get_visible_items();
    let has_ces_group = items
        .iter()
        .any(|i| matches!(i, ConfigItem::Group(n, ns) if n == "CES_Group" && ns == "CES"));
    assert!(
        !has_ces_group,
        "CES_Group should be hidden since it has no favorites"
    );
}

// ── namespace + env + favorites ───────────────────────────────────────────────

#[test]
fn namespace_env_favorite_shows_env_and_server() {
    let config = make_namespace_env_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.favorites.clear();

    let ns_server = app.resolved_servers[0].clone();
    app.favorites.insert(App::server_key(&ns_server));
    app.favorites_only = true;

    // Expand everything.
    app.expanded_items.insert("NS:NS1".to_string());
    app.expanded_items.insert("NS:NS1:Group:GrpA".to_string());
    app.expanded_items
        .insert("NS:NS1:Env:GrpA:EnvA".to_string());
    app.items_dirty = true;

    let items = app.get_visible_items();

    let has_server = items
        .iter()
        .any(|i| matches!(i, ConfigItem::Server(s) if s.name == "ns_srv"));
    assert!(has_server, "ns_srv should be visible when favorited");
}

#[test]
fn namespace_env_expand_all_covers_ns_env_key() {
    let config = make_namespace_env_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.expanded_items.clear();

    app.expand_all();

    assert!(app.expanded_items.contains("NS:NS1"));
    assert!(app.expanded_items.contains("NS:NS1:Group:GrpA"));
    assert!(app.expanded_items.contains("NS:NS1:Env:GrpA:EnvA"));
}

#[test]
fn toggle_expansion_on_namespaced_env() {
    let config = make_namespace_env_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    // Expand NS and group first.
    app.expanded_items.insert("NS:NS1".to_string());
    app.expanded_items.insert("NS:NS1:Group:GrpA".to_string());
    app.items_dirty = true;
    app.get_visible_items();

    // Items: [NS:NS1, Group:GrpA(NS1), Env:GrpA:EnvA(NS1)]
    let items = app.get_visible_items();
    let env_idx = items
        .iter()
        .position(|i| matches!(i, ConfigItem::Environment(g, e, ns) if g == "GrpA" && e == "EnvA" && ns == "NS1"))
        .unwrap();
    app.select(env_idx);
    app.toggle_expansion();

    // Should insert the namespaced env key.
    assert!(app.expanded_items.contains("NS:NS1:Env:GrpA:EnvA"));
}

// ── toggle_expansion edge cases ───────────────────────────────────────────────

#[test]
fn toggle_expansion_on_server_is_noop() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    // Expand everything first.
    app.expanded_items.insert("Group:G1".to_string());
    app.expanded_items.insert("Env:G1:E1".to_string());
    app.items_dirty = true;
    app.get_visible_items();

    // Select the server item (S2 is at index 3: Group, Env, S1, S2).
    app.select(3);
    let items = app.get_visible_items();
    assert!(matches!(items.get(3), Some(ConfigItem::Server(_))));

    let expanded_before = app.expanded_items.clone();
    app.toggle_expansion();
    // Expanding on a server should not change expanded_items.
    assert_eq!(app.expanded_items, expanded_before);
}

#[test]
fn toggle_expansion_on_namespace_group_uses_namespaced_key() {
    let config = make_namespace_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.expanded_items.clear();
    app.items_dirty = true;
    app.get_visible_items();

    // Expand the CES namespace first (index 1 in the collapsed list).
    app.select(1);
    app.toggle_expansion();

    let items = app.get_visible_items();
    // Now the CES_Group should be visible at index 2.
    assert!(
        items
            .iter()
            .any(|i| matches!(i, ConfigItem::Group(n, ns) if n == "CES_Group" && ns == "CES"))
    );

    // Select the CES_Group and expand it — uses the "NS:CES:Group:CES_Group" key.
    let group_idx = items
        .iter()
        .position(|i| matches!(i, ConfigItem::Group(n, ns) if n == "CES_Group" && ns == "CES"))
        .unwrap();
    app.select(group_idx);
    app.toggle_expansion();
    assert!(app.expanded_items.contains("NS:CES:Group:CES_Group"));
}

// ── favorites_only mode ───────────────────────────────────────────────────────

#[test]
fn favorites_only_hides_non_favorite_servers() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    // Expand so servers are visible.
    app.expanded_items.insert("Group:G1".to_string());
    app.expanded_items.insert("Env:G1:E1".to_string());
    app.items_dirty = true;

    // Enable favorites mode without adding any favorites.
    app.favorites.clear();
    app.favorites_only = true;
    app.items_dirty = true;

    let items = app.get_visible_items();
    // No favorites → nothing visible.
    assert!(items.is_empty());
}

#[test]
fn favorites_only_shows_only_favorited_server() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.expanded_items.insert("Group:G1".to_string());
    app.expanded_items.insert("Env:G1:E1".to_string());
    app.items_dirty = true;
    app.get_visible_items();

    // Favorite only S2 (the server directly under G1, not inside E1).
    let server_s2 = app
        .resolved_servers
        .iter()
        .find(|s| s.name == "S2")
        .cloned()
        .unwrap();
    app.favorites.insert(App::server_key(&server_s2));
    app.favorites_only = true;
    app.items_dirty = true;

    let items = app.get_visible_items();
    let names: Vec<&str> = items
        .iter()
        .filter_map(|i| {
            if let ConfigItem::Server(s) = i {
                Some(s.name.as_str())
            } else {
                None
            }
        })
        .collect();
    assert!(names.contains(&"S2"), "S2 should be visible");
    assert!(!names.contains(&"S1"), "S1 should be hidden");
}

// ── sort_by_recent mode ───────────────────────────────────────────────────────

#[test]
fn sort_by_recent_returns_flat_list_of_servers() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();
    app.sort_by_recent = true;
    app.items_dirty = true;

    let items = app.get_visible_items();
    // All items should be servers (flat list).
    assert!(!items.is_empty());
    assert!(items.iter().all(|i| matches!(i, ConfigItem::Server(_))));
}

#[test]
fn sort_by_recent_orders_by_timestamp_descending() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    // Give S1 a newer timestamp than S2.
    let s1 = app
        .resolved_servers
        .iter()
        .find(|s| s.name == "S1")
        .cloned()
        .unwrap();
    let s2 = app
        .resolved_servers
        .iter()
        .find(|s| s.name == "S2")
        .cloned()
        .unwrap();
    app.last_seen.insert(App::server_key(&s1), 2000);
    app.last_seen.insert(App::server_key(&s2), 1000);

    app.sort_by_recent = true;
    app.items_dirty = true;

    let items = app.get_visible_items();
    let names: Vec<&str> = items
        .iter()
        .filter_map(|i| {
            if let ConfigItem::Server(s) = i {
                Some(s.name.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(names[0], "S1", "S1 (newer) should come first");
    assert_eq!(names[1], "S2");
}

#[test]
fn sort_by_recent_respects_favorites_only() {
    let config = create_test_config();
    let mut app = App::new(config, vec![], std::path::PathBuf::new(), vec![]).unwrap();

    app.favorites.clear();
    let s2 = app
        .resolved_servers
        .iter()
        .find(|s| s.name == "S2")
        .cloned()
        .unwrap();
    app.favorites.insert(App::server_key(&s2));

    app.sort_by_recent = true;
    app.favorites_only = true;
    app.items_dirty = true;

    let items = app.get_visible_items();
    assert_eq!(items.len(), 1);
    if let ConfigItem::Server(s) = &items[0] {
        assert_eq!(s.name, "S2");
    }
}
