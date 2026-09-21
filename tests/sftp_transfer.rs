/// Test d'intégration SFTP — nécessite un serveur SSH réel sur 192.0.2.13.
/// Lancer avec : cargo test --test sftp_transfer -- --nocapture --ignored
///
/// Le serveur cible doit être joignable et l'agent SSH ou ~/.ssh/id_ed25519 doivent
/// permettre une connexion ops-user@192.0.2.13.
use std::time::Duration;
use susshi::config::{ConnectionMode, ResolvedServer};
use susshi::ssh::sftp::{ScpDirection, ScpEvent, spawn_sftp};

fn test_server() -> ResolvedServer {
    ResolvedServer {
        namespace: String::new(),
        group_name: String::new(),
        env_name: String::new(),
        name: "test-sftp".into(),
        host: "192.0.2.13".into(),
        user: "root".into(),
        port: 22,
        ssh_key: String::new(), // agent SSH ou clés par défaut
        ssh_options: vec![],
        default_mode: ConnectionMode::Direct,
        jump_host: None,
        bastion_host: None,
        bastion_user: None,
        bastion_template: String::new(),
        use_system_ssh_config: false,
        probe_filesystems: vec![],
        tunnels: vec![],
        tags: vec![],
        control_master: false,
        agent_forwarding: false,
        control_path: String::new(),
        control_persist: "10m".to_string(),
        pre_connect_hook: None,
        post_disconnect_hook: None,
        hook_timeout_secs: 5,
        ssh_cert: String::new(),
        notes: String::new(),
        production: false,
        confirm_production: false,
        ssh_agent_sock: String::new(),
        wallix_group: None,
        wallix_account: "default".to_string(),
        wallix_protocol: "SSH".to_string(),
        wallix_auto_select: true,
        wallix_fail_if_menu_match_error: true,
        wallix_selection_timeout_secs: 8,
        wallix_direct: false,
        wallix_authorization: None,
        wallix_header_columns: vec![],
    }
}

/// Upload /tmp/random-data.bin → ops-user@192.0.2.13:/root/random-data.bin
/// Vérifie que tous les événements ScpEvent::Progress arrivent dans l'ordre
/// et que ScpEvent::Done(true) est bien reçu.
#[test]
#[ignore]
fn upload_random_data_to_192_0_2_13() {
    let local = "/tmp/random-data.bin";
    assert!(
        std::path::Path::new(local).exists(),
        "Fichier de test manquant : {local}\nCréer avec : dd if=/dev/urandom of={local} bs=1M count=2"
    );

    let server = test_server();
    let rx = spawn_sftp(
        &server,
        ConnectionMode::Direct,
        ScpDirection::Upload,
        local,
        "/root/random-data.bin",
    )
    .expect("spawn_sftp ne doit pas échouer immédiatement");

    let mut last_pct: i64 = -1;

    loop {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(ScpEvent::FileSize(sz)) => {
                println!("FileSize: {sz} bytes");
            }
            Ok(ScpEvent::Progress(pct)) => {
                println!("Progress: {pct}%");
                assert!(
                    pct as i64 >= last_pct,
                    "progression régressive : {pct} < {last_pct}"
                );
                last_pct = pct as i64;
            }
            Ok(ScpEvent::Done(success)) => {
                println!("Done: success={success}");
                assert!(success, "transfert signalé échoué (Done(false))");
                break;
            }
            Ok(ScpEvent::Error(e)) => {
                panic!("ScpEvent::Error reçu : {e}");
            }
            Err(e) => {
                panic!("Timeout ou channel fermé : {e}");
            }
        }
    }

    assert_eq!(last_pct, 100, "progression n'a pas atteint 100%");
    println!("Upload OK — progression finale : {last_pct}%");
}

/// Download ops-user@192.0.2.13:~/random-data.bin → /tmp/random-data-downloaded.bin
/// Vérifie que le fichier local est créé et a la même taille que l'original.
#[test]
#[ignore]
fn download_random_data_from_192_0_2_13() {
    let local_dest = "/tmp/random-data-downloaded.bin";
    // On s'assure que le fichier de destination n'existe pas avant le test.
    let _ = std::fs::remove_file(local_dest);

    let server = test_server();
    let rx = spawn_sftp(
        &server,
        ConnectionMode::Direct,
        ScpDirection::Download,
        local_dest,
        "~/random-data.bin",
    )
    .expect("spawn_sftp ne doit pas échouer immédiatement");

    let mut last_pct: i64 = -1;

    loop {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(ScpEvent::FileSize(sz)) => {
                println!("FileSize: {sz} bytes");
            }
            Ok(ScpEvent::Progress(pct)) => {
                println!("Progress: {pct}%");
                assert!(pct as i64 >= last_pct, "progression régressive");
                last_pct = pct as i64;
            }
            Ok(ScpEvent::Done(success)) => {
                println!("Done: success={success}");
                assert!(success, "transfert signalé échoué (Done(false))");
                break;
            }
            Ok(ScpEvent::Error(e)) => panic!("ScpEvent::Error reçu : {e}"),
            Err(e) => panic!("Timeout ou channel fermé : {e}"),
        }
    }

    assert_eq!(last_pct, 100, "progression n'a pas atteint 100%");
    // Vérifie que le fichier a bien été créé et fait ~2 Mo
    let meta = std::fs::metadata(local_dest).expect("fichier téléchargé manquant");
    assert!(
        meta.len() > 1_000_000,
        "fichier téléchargé trop petit : {} octets",
        meta.len()
    );
    println!("Download OK — taille : {} octets", meta.len());
}

/// Vérifie que download vers /tmp (répertoire) crée /tmp/random-data.bin
/// et non une erreur EISDIR.
#[test]
#[ignore]
fn download_to_directory_appends_filename() {
    let local_dir = "/tmp";
    let expected = "/tmp/random-data.bin";
    let _ = std::fs::remove_file(expected);

    let server = test_server();
    let rx = spawn_sftp(
        &server,
        ConnectionMode::Direct,
        ScpDirection::Download,
        local_dir,
        "~/random-data.bin",
    )
    .expect("spawn_sftp ne doit pas échouer immédiatement");

    loop {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(ScpEvent::Done(success)) => {
                assert!(success);
                break;
            }
            Ok(ScpEvent::Error(e)) => panic!("ScpEvent::Error : {e}"),
            Err(e) => panic!("Timeout : {e}"),
            _ => {}
        }
    }

    assert!(
        std::path::Path::new(expected).exists(),
        "fichier attendu non créé : {expected}"
    );
    println!("Download vers répertoire OK → {expected}");
}
