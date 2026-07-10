use super::*;

#[test]
fn mint_token_is_nonempty_hex() {
    let t = mint_token();
    assert_eq!(t.len(), 32);
    assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(mint_token(), mint_token());
}

#[test]
fn reserved_port_is_free_for_the_child() {
    let reservation = reserve_port().unwrap();
    let port = reservation.local_addr().unwrap().port();
    assert_ne!(port, 0);
    // Once the reservation is dropped, the child can bind the port immediately.
    drop(reservation);
    std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
}

#[test]
fn wait_ready_fails_when_the_port_is_squatted_and_the_child_exits() {
    // Someone else owns the port; our child (per the sidecar contract) exits
    // when it can't bind. The boot must fail rather than trust the squatter.
    let squatter = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = squatter.local_addr().unwrap().port();
    let mut child = std::process::Command::new("sh")
        .args(["-c", "sleep 0.2; exit 1"])
        .spawn()
        .unwrap();
    let err = wait_ready(&mut child, port).unwrap_err();
    assert!(
        err.contains("something else may be listening") || err.contains("exited"),
        "{err}"
    );
}

#[test]
fn acquire_reports_a_service_that_dies_at_startup() {
    let dir = std::env::temp_dir().join("sinclairsidecarexit");
    std::fs::create_dir_all(&dir).unwrap();
    let err = acquire("test.exit", "false", &dir).unwrap_err();
    assert!(err.contains("exited during startup"), "{err}");
    // A failed start takes no reference and leaves nothing registered.
    assert!(!SERVICES.lock().unwrap().contains_key("test.exit"));
}

#[test]
fn acquire_rejects_an_empty_command() {
    let dir = std::env::temp_dir();
    let err = acquire("test.empty", "  ", &dir).unwrap_err();
    assert_eq!(err, "empty service command");
}

#[test]
fn release_of_an_unknown_id_is_a_noop() {
    release("test.never-acquired");
}

#[test]
fn resolve_program_keeps_paths_and_unknown_names() {
    assert_eq!(resolve_program("/usr/bin/env"), "/usr/bin/env");
    assert_eq!(resolve_program("definitelynotasibling"), "definitelynotasibling");
}
