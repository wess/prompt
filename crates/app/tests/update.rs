use super::*;

#[test]
fn newer_versions_are_detected() {
    assert!(is_newer("1.21.0", "1.20.0"));
    assert!(is_newer("2.0.0", "1.99.99"));
    assert!(is_newer("1.20.1", "1.20.0"));
    assert!(is_newer("v1.21.0", "1.20.0")); // tolerates leading v
}

#[test]
fn same_or_older_is_not_newer() {
    assert!(!is_newer("1.20.0", "1.20.0"));
    assert!(!is_newer("1.19.0", "1.20.0"));
    assert!(!is_newer("nonsense", "1.20.0"));
    assert!(!is_newer("1.20.0", "garbage"));
}

#[test]
fn release_asset_lookup() {
    let r = Release {
        version: "1.21.0".into(),
        url: "https://x".into(),
        assets: vec![
            ("Prompt.dmg".into(), "https://d/Prompt.dmg".into()),
            ("prompt_1.21.0_arm64.deb".into(), "https://d/deb".into()),
            ("Prompt-1.21.0-aarch64.AppImage".into(), "https://d/img".into()),
        ],
    };
    assert_eq!(r.asset(".dmg"), Some("https://d/Prompt.dmg"));
    assert_eq!(r.asset(".AppImage"), Some("https://d/img"));
    assert_eq!(r.asset(".exe"), None);
}

#[test]
fn only_swappable_installs_update_in_place() {
    // macOS .app and Linux AppImage are rewritten in place; everything else
    // (a root-owned distro package, a dev build) opens the download page.
    assert!(Install::MacApp(std::path::PathBuf::from("/Applications/Prompt.app")).is_in_place());
    assert!(Install::AppImage(std::path::PathBuf::from("/x/Prompt.AppImage")).is_in_place());
    assert!(!Install::Unknown.is_in_place());
}
