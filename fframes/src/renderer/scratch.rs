//! The folder a render keeps its encoded segments in until they are concatenated.
//!
//! A [`ScratchDir`] the renderer creates is removed when the guard drops: after the
//! output is written, and on every error or panic that leaves the render. A folder the
//! caller passed in (`RenderOptions::tmp_files_directory`) is the caller's and is kept.
//!
//! Scratch folders live under `FFRAMES_TMP_DIR` when it is set, else the system temp
//! folder. A process that is killed leaves its folder behind, so the first scratch folder a
//! process makes also sweeps the root for abandoned ones: `fframes-*` render folders nothing
//! has written to for [`STALE_AFTER`]. A render that is still running writes its segments
//! continuously, so its folder is never that old.

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, SystemTime};

/// The variable that moves scratch folders off the system temp folder.
pub const TMP_DIR_VAR: &str = "FFRAMES_TMP_DIR";

/// How long a render folder can go unwritten before a sweep removes it.
pub const STALE_AFTER: Duration = Duration::from_secs(6 * 60 * 60);

/// The prefixes of render folders: `fframes-skia-<uuid>` (GPU) and `fframes-<uuid>` (CPU).
const PREFIXES: &[&str] = &["fframes-skia-", "fframes-"];

/// Where scratch folders are made.
pub fn scratch_root() -> PathBuf {
    std::env::var_os(TMP_DIR_VAR)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// A render's segment folder, removed on drop when the renderer made it.
pub struct ScratchDir {
    path: PathBuf,
    owned: bool,
}

impl ScratchDir {
    /// The caller's folder when given (created if missing, kept afterwards), else a new
    /// `<prefix><uuid>` folder under [`scratch_root`], removed on drop.
    pub fn new(prefix: &str, caller: Option<&PathBuf>) -> std::io::Result<Self> {
        static SWEEP: Once = Once::new();
        let root = scratch_root();
        if caller.is_none() {
            SWEEP.call_once(|| {
                sweep(&root, STALE_AFTER, SystemTime::now());
            });
        }
        Self::new_in(&root, prefix, caller)
    }

    /// [`ScratchDir::new`] under a given root, without the sweep.
    fn new_in(root: &Path, prefix: &str, caller: Option<&PathBuf>) -> std::io::Result<Self> {
        if let Some(dir) = caller {
            if !dir.exists() {
                std::fs::create_dir_all(dir)?;
            }
            return Ok(Self {
                path: dir.clone(),
                owned: false,
            });
        }
        if !root.exists() {
            std::fs::create_dir_all(root)?;
        }
        let path = root.join(format!("{prefix}{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path)?;
        Ok(Self { path, owned: true })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// The folder when it outlives the render (the caller's), for the success message.
    pub fn kept(&self) -> Option<&PathBuf> {
        (!self.owned).then_some(&self.path)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        if self.owned {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// Whether a folder name is a render folder: a prefix followed by a uuid.
fn is_render_folder(name: &str) -> bool {
    PREFIXES.iter().any(|p| {
        name.strip_prefix(p)
            .is_some_and(|rest| uuid::Uuid::try_parse(rest).is_ok())
    })
}

/// The newest modification time of a folder and everything in it.
fn newest(path: &Path) -> Option<SystemTime> {
    let mut latest = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if let Ok(t) = meta.modified() {
                latest = latest.max(t);
            }
            if meta.is_dir() {
                stack.push(entry.path());
            }
        }
    }
    Some(latest)
}

/// Removes the render folders under `root` that nothing has written to for `stale`.
/// Returns how many it removed; errors (a folder another process is deleting or still
/// holds open) leave that folder alone.
pub fn sweep(root: &Path, stale: Duration, now: SystemTime) -> usize {
    let Ok(entries) = std::fs::read_dir(root) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_render_folder(name) || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.path();
        let old = newest(&path)
            .and_then(|t| now.duration_since(t).ok())
            .is_some_and(|age| age >= stale);
        if old && std::fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scratch-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_owned_folder_goes_when_the_guard_drops() {
        let base = root("drop");
        let dir = {
            let scratch = ScratchDir::new_in(&base, "fframes-skia-", None).unwrap();
            std::fs::write(scratch.path().join("0.mp4"), b"x").unwrap();
            assert!(scratch.kept().is_none());
            scratch.path().clone()
        };
        assert!(!dir.exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_folder_goes_when_the_render_fails() {
        fn render(base: &Path, dir: &mut Option<PathBuf>) -> std::io::Result<()> {
            let scratch = ScratchDir::new_in(base, "fframes-skia-", None)?;
            *dir = Some(scratch.path().clone());
            Err(std::io::Error::other("encoder failed"))
        }
        let base = root("fail");
        let mut dir = None;
        assert!(render(&base, &mut dir).is_err());
        assert!(!dir.unwrap().exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn the_callers_folder_stays() {
        let base = root("caller");
        let chunks = base.join("chunks");
        {
            let scratch = ScratchDir::new_in(&base, "fframes-skia-", Some(&chunks)).unwrap();
            assert_eq!(scratch.kept(), Some(&chunks));
        }
        assert!(chunks.exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn the_sweep_takes_only_stale_render_folders() {
        let base = root("sweep");
        let id = uuid::Uuid::new_v4();
        let skia = base.join(format!("fframes-skia-{id}"));
        let cpu = base.join(format!("fframes-{id}"));
        let other = base.join("fframes-smoke-1234");
        let unrelated = base.join("something-else");
        for d in [&skia, &cpu, &other, &unrelated] {
            std::fs::create_dir(d).unwrap();
            std::fs::write(d.join("0.mp4"), b"x").unwrap();
        }
        // Everything was just written: nothing is stale.
        assert_eq!(sweep(&base, STALE_AFTER, SystemTime::now()), 0);
        assert!(skia.exists() && cpu.exists());
        // Seven hours on, the two render folders are abandoned; the rest are not ours.
        let later = SystemTime::now() + Duration::from_secs(7 * 60 * 60);
        assert_eq!(sweep(&base, STALE_AFTER, later), 2);
        assert!(!skia.exists() && !cpu.exists());
        assert!(other.exists() && unrelated.exists());
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_recent_file_keeps_an_old_folder() {
        let base = root("live");
        let live = base.join(format!("fframes-skia-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(live.join("nested")).unwrap();
        std::fs::write(live.join("nested").join("7.mp4"), b"x").unwrap();
        let file_time = newest(&live).unwrap();
        // Five hours after the newest write the render may still be going.
        let now = file_time + Duration::from_secs(5 * 60 * 60);
        assert_eq!(sweep(&base, STALE_AFTER, now), 0);
        assert!(live.exists());
        std::fs::remove_dir_all(base).unwrap();
    }
}
