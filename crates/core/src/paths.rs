//! Where the app keeps its data (config now, logs later).
//!
//! Two modes, chosen at runtime with zero build-time divergence:
//! - **Portable**: a `portable.txt` marker sits next to the executable →
//!   data lives in the exe's own folder, so the whole folder is copy-portable.
//! - **Installed**: no marker → data lives in the per-user OS config dir
//!   (`app_config_dir()`), the original behavior.
//!
//! The decision is a pure function of two paths, so it unit-tests on any OS
//! without Tauri.

use std::path::{Path, PathBuf};

/// Marker file whose PRESENCE next to the exe switches on portable mode.
/// Contents are ignored. Shipped (empty) inside the portable zip; never
/// installed, so an installed build keeps its `%APPDATA%` behavior.
pub const PORTABLE_MARKER: &str = "portable.txt";

/// True when a `portable.txt` file sits next to the executable (a directory
/// of that name does not count).
pub fn is_portable(exe_dir: &Path) -> bool {
    exe_dir.join(PORTABLE_MARKER).is_file()
}

/// The data base directory: the exe's own folder in portable mode, else the
/// installed per-user config dir. `config.toml` (and future logs) go under it.
pub fn base_dir(exe_dir: &Path, installed_dir: &Path) -> PathBuf {
    if is_portable(exe_dir) {
        exe_dir.to_path_buf()
    } else {
        installed_dir.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn not_portable_without_marker() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_portable(dir.path()));
    }

    #[test]
    fn portable_with_marker() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(PORTABLE_MARKER), "").unwrap();
        assert!(is_portable(dir.path()));
    }

    #[test]
    fn marker_content_is_ignored() {
        // Existence-only: a marker with arbitrary content still counts.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(PORTABLE_MARKER), "anything at all").unwrap();
        assert!(is_portable(dir.path()));
    }

    #[test]
    fn base_dir_is_exe_dir_in_portable_mode() {
        let exe = tempfile::tempdir().unwrap();
        let installed = tempfile::tempdir().unwrap();
        fs::write(exe.path().join(PORTABLE_MARKER), "").unwrap();
        assert_eq!(base_dir(exe.path(), installed.path()).as_path(), exe.path());
    }

    #[test]
    fn base_dir_is_installed_dir_without_marker() {
        let exe = tempfile::tempdir().unwrap();
        let installed = tempfile::tempdir().unwrap();
        assert_eq!(
            base_dir(exe.path(), installed.path()).as_path(),
            installed.path()
        );
    }
}
