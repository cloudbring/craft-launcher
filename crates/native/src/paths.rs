//! Where the launcher keeps its own files.

use std::path::PathBuf;

/// The launcher's folders.
#[derive(Clone, Debug)]
pub struct Paths {
    /// Records of what the launcher installed, and tarball installs.
    pub data: PathBuf,
    /// Saved release answers and in-progress downloads.
    pub cache: PathBuf,
}

const DIR: &str = if cfg!(any(target_os = "macos", target_os = "windows")) { "ArtCraft Launcher" } else { "artcraft-launcher" };

impl Paths {
    pub fn new() -> Result<Self, String> {
        if let Some(root) = portable_root() {
            return Ok(Self::under(&root));
        }
        let data = dirs::data_local_dir().ok_or("no per-user data folder on this system")?.join(DIR);
        let cache = dirs::cache_dir().ok_or("no per-user cache folder on this system")?.join(DIR);
        Ok(Self { data, cache })
    }

    /// For tests: everything under one folder.
    pub fn under(root: &std::path::Path) -> Self {
        Self { data: root.join("data"), cache: root.join("cache") }
    }

    pub fn downloads(&self) -> PathBuf {
        self.cache.join("downloads")
    }

    pub fn release_cache(&self) -> PathBuf {
        self.cache.join("releases.json")
    }

    /// The last answer from ArtCraft's release feed, for an instant first paint and as a fallback.
    pub fn feed_cache(&self) -> PathBuf {
        self.cache.join("feed.json")
    }

    /// How long to hold off calling ArtCraft's API (after failures or HTTP 429), across restarts.
    pub fn feed_throttle(&self) -> PathBuf {
        self.cache.join("feed-throttle.json")
    }

    /// A picture of an app's details page, saved after its first download: under the revision of the
    /// pictures repository it was fetched from, so a new revision never reads an old picture.
    pub fn screenshot(&self, app: &str, key: &str) -> PathBuf {
        self.cache.join("screenshots").join(artcraft_launcher_core::details::SCREENSHOTS_REV).join(app).join(format!("{key}.jpg"))
    }

    pub fn manifest(&self) -> PathBuf {
        self.data.join("installed.json")
    }

    /// The launcher's own log files (`artcraft-launcher.log` and the previous runs').
    pub fn logs(&self) -> PathBuf {
        self.data.join("logs")
    }

    /// Where tarball installs (FreeBSD, Linux without AppImage) are unpacked: `<data>/apps/<app>`.
    pub fn apps(&self) -> PathBuf {
        self.data.join("apps")
    }

    /// The folder an app unpacked in [`Self::apps`] sees as its home: `<data>/apps/<app>/home`.
    /// A Craft App keeps its settings in the XDG folders (`~/.config`, `~/.local/share`,
    /// `~/.cache`), so pointing `HOME` and those variables here keeps every app's configuration
    /// beside it, inside the folder the launcher owns.
    #[cfg(target_os = "linux")]
    pub fn app_home(&self, id: &str) -> PathBuf {
        self.apps().join(id).join("home")
    }
}

/// Writes `bytes` to `path` via a temporary file and a rename, so a crash never leaves half a file.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic_with_fs(&craft_atomic::RealFs, path, bytes)
}

fn write_atomic_with_fs(fs: &dyn craft_atomic::AtomicFs, path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path.parent().ok_or("no parent folder")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("couldn't create {} ({e})", dir.display()))?;
    craft_atomic::atomic_write_custom(fs, craft_atomic::RenameRetry::platform(), path, bytes)
        .map_err(|e| format!("couldn't replace {} ({e})", path.display()))
}

/// Portable mode: a `portable.txt` beside the executable keeps everything in
/// `ArtCraftLauncherData` next to it (the Windows portable zip ships one).
pub fn portable_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    dir.join("portable.txt").is_file().then(|| dir.join("ArtCraftLauncherData"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use craft_atomic::{AtomicFs, NewFilePermissions, RealFs};
    use std::fs::File;
    use std::io;
    use std::path::Path;

    struct FailingSync;

    impl AtomicFs for FailingSync {
        fn create_new(&self, path: &Path, permissions: NewFilePermissions) -> io::Result<File> {
            RealFs.create_new(path, permissions)
        }

        fn sync(&self, _file: &File) -> io::Result<()> {
            Err(io::Error::other("injected sync failure"))
        }

        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            RealFs.rename(from, to)
        }

        fn hard_link(&self, from: &Path, to: &Path) -> io::Result<()> {
            RealFs.hard_link(from, to)
        }

        fn remove(&self, path: &Path) -> io::Result<()> {
            RealFs.remove(path)
        }

        fn sync_dir(&self, dir: &Path) -> io::Result<()> {
            RealFs.sync_dir(dir)
        }

        fn set_permissions(&self, file: &File, permissions: std::fs::Permissions) -> io::Result<()> {
            RealFs.set_permissions(file, permissions)
        }
    }

    #[test]
    fn atomic_write_syncs_before_replacing_the_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("installed.json");
        std::fs::write(&path, b"old manifest").unwrap();

        let error = write_atomic_with_fs(&FailingSync, &path, b"new manifest").unwrap_err();
        assert!(error.contains("injected sync failure"));
        assert_eq!(std::fs::read(&path).unwrap(), b"old manifest");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn atomic_write_does_not_overwrite_an_unrelated_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("installed.json");
        let unrelated = path.with_extension("tmp");
        std::fs::write(&unrelated, b"another writer's data").unwrap();
        write_atomic(&path, b"new manifest").unwrap();
        assert_eq!(std::fs::read(&unrelated).unwrap(), b"another writer's data");
        assert_eq!(std::fs::read(&path).unwrap(), b"new manifest");
    }

    #[test]
    fn atomic_write_replaces_a_complete_file_and_cleans_up_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("installed.json");
        std::fs::write(&path, b"old manifest").unwrap();
        write_atomic(&path, b"new manifest").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new manifest");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);

        let blocked = dir.path().join("directory.json");
        std::fs::create_dir(&blocked).unwrap();
        std::fs::write(blocked.join("keep.txt"), b"keep").unwrap();
        assert!(write_atomic(&blocked, b"replacement").is_err());
        assert_eq!(std::fs::read(blocked.join("keep.txt")).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_does_not_follow_a_temporary_file_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("installed.json");
        let victim = dir.path().join("unrelated.txt");
        std::fs::write(&victim, b"keep this file").unwrap();
        std::os::unix::fs::symlink(&victim, path.with_extension("tmp")).unwrap();
        write_atomic(&path, b"new manifest").unwrap();
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep this file");
        assert_eq!(std::fs::read(&path).unwrap(), b"new manifest");
    }
}
