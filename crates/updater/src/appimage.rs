//! Linux AppImage: download the new image and rename it over the running one.
//! The running process keeps its open inode; the next launch gets the new file.

use std::path::Path;

use crate::{fetch, Relaunch, Release};

/// Download the release's AppImage and swap it in, returning the relaunch
/// target (the image path, handed to `gpui::App::set_restart_path`).
pub(crate) fn install(release: &Release, target: &Path) -> Result<Relaunch, String> {
    let url = release.asset(".AppImage").ok_or("release has no AppImage asset")?;
    // Stage *next to* the target, not in the temp dir: the final rename must
    // not cross filesystems (`/tmp` is often tmpfs), or it fails with EXDEV.
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("Sinclair.AppImage");
    let staged = target.with_file_name(format!(".{name}.update"));
    if let Err(e) = fetch::file(url, &staged) {
        // A dead download must not strand a partial image next to the app.
        let _ = std::fs::remove_file(&staged);
        return Err(e);
    }
    promote(&staged, target)
}

/// Mark `staged` executable and rename it over `target`, dropping the staged
/// file if the rename fails.
fn promote(staged: &Path, target: &Path) -> Result<Relaunch, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(staged, std::fs::Permissions::from_mode(0o755));
    }
    if let Err(e) = std::fs::rename(staged, target) {
        let _ = std::fs::remove_file(staged);
        return Err(format!("replace AppImage: {e}"));
    }
    Ok(Relaunch::Binary(target.to_path_buf()))
}

#[cfg(test)]
#[path = "../tests/appimage.rs"]
mod tests;
