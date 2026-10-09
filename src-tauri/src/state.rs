use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use nimata_core::archive::format_utc;
use nimata_core::attachments::BlobStore;
use nimata_core::{Repository, SqliteRepository, UnixMillis};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

pub const DATABASE_FILE: &str = "nimata.sqlite3";

/// Folder inside the data folder for automatic safety copies.
pub const SAFETY_COPIES: &str = "safety-copies";

pub struct AppState {
    inner: Mutex<Inner>,
    /// The data folder, when it could be determined.
    pub data_dir: Option<PathBuf>,
    /// Where attached files are kept: `blobs` in the data folder.
    blobs: Option<BlobStore>,
}

/// Folder inside the data folder for attached files.
pub const BLOBS: &str = "blobs";

struct Inner {
    /// The open database. `None` when it could not be opened; `error` then
    /// says why, and the UI offers to restore a backup instead of crashing.
    repo: Option<SqliteRepository>,
    error: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub path: Option<String>,
    /// Why the database could not be opened, if it could not.
    pub error: Option<String>,
}

/// Exclusive access to the open database.
pub struct RepoGuard<'a>(MutexGuard<'a, Inner>);

impl Deref for RepoGuard<'_> {
    type Target = SqliteRepository;
    fn deref(&self) -> &SqliteRepository {
        self.0
            .repo
            .as_ref()
            .expect("checked when the guard was created")
    }
}

impl DerefMut for RepoGuard<'_> {
    fn deref_mut(&mut self) -> &mut SqliteRepository {
        self.0
            .repo
            .as_mut()
            .expect("checked when the guard was created")
    }
}

/// A file name fragment for the current time, e.g. `2026-10-05T14-03-22Z`.
fn timestamp_for_files() -> String {
    // format_utc gives "2026-10-05T14:03:22.123Z"; keep it to the second.
    let utc = format_utc(UnixMillis::now());
    format!("{}Z", utc[..19].replace(':', "-"))
}

impl AppState {
    pub fn open<R: Runtime>(app: &AppHandle<R>) -> Self {
        match app.path().app_data_dir() {
            Ok(dir) => Self::open_in(&dir),
            Err(e) => Self::failed(format!("cannot locate the data folder: {e}"), None),
        }
    }

    /// Opens or creates the database in `dir`, creating the folder if needed.
    pub fn open_in(dir: &Path) -> Self {
        if let Err(e) = std::fs::create_dir_all(dir) {
            return Self::failed(format!("cannot create {}: {e}", dir.display()), None);
        }
        let path = dir.join(DATABASE_FILE);
        match SqliteRepository::open(&path) {
            Ok(mut repo) => {
                // Replies still being written when Nimata last stopped cannot
                // resume; record them as interrupted, keeping their text.
                let _ = repo.recover_interrupted(UnixMillis::now());
                Self {
                    inner: Mutex::new(Inner {
                        repo: Some(repo),
                        error: None,
                    }),
                    data_dir: Some(dir.to_path_buf()),
                    blobs: Some(BlobStore::new(dir.join(BLOBS))),
                }
            }
            Err(e) => Self::failed(
                format!("cannot open {}: {e}", path.display()),
                Some(dir.to_path_buf()),
            ),
        }
    }

    /// An in-memory database, with attached files in a fresh temporary
    /// folder.
    #[cfg(test)]
    pub fn with_repo(repo: SqliteRepository) -> Self {
        let blobs =
            std::env::temp_dir().join(format!("nimata-test-blobs-{}", uuid::Uuid::now_v7()));
        Self {
            inner: Mutex::new(Inner {
                repo: Some(repo),
                error: None,
            }),
            data_dir: None,
            blobs: Some(BlobStore::new(blobs)),
        }
    }

    fn failed(message: String, data_dir: Option<PathBuf>) -> Self {
        Self {
            inner: Mutex::new(Inner {
                repo: None,
                error: Some(message),
            }),
            blobs: data_dir.as_ref().map(|d| BlobStore::new(d.join(BLOBS))),
            data_dir,
        }
    }

    pub fn blobs(&self) -> Result<&BlobStore, String> {
        self.blobs
            .as_ref()
            .ok_or_else(|| "the data folder is unknown, so files cannot be stored".to_string())
    }

    /// Removes the stored bytes of `hashes` that nothing refers to any more.
    pub fn release(&self, hashes: impl IntoIterator<Item = String>) -> Result<(), String> {
        let store = self.blobs()?;
        let mut repo = self.repo()?;
        for hash in hashes {
            if !repo.hash_in_use(&hash).map_err(|e| e.to_string())? {
                store.remove(&hash).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, String> {
        self.inner
            .lock()
            .map_err(|_| "the database is unavailable after an earlier failure".to_string())
    }

    pub fn database_path(&self) -> Option<PathBuf> {
        self.data_dir.as_ref().map(|dir| dir.join(DATABASE_FILE))
    }

    pub fn status(&self) -> DatabaseStatus {
        let error = match self.lock() {
            Ok(inner) => inner.error.clone(),
            Err(e) => Some(e),
        };
        DatabaseStatus {
            path: self.database_path().map(|p| p.display().to_string()),
            error,
        }
    }

    pub fn repo(&self) -> Result<RepoGuard<'_>, String> {
        let inner = self.lock()?;
        if inner.repo.is_none() {
            return Err(inner
                .error
                .clone()
                .unwrap_or_else(|| "the database is not open".to_string()));
        }
        Ok(RepoGuard(inner))
    }

    /// Writes a backup of the open database to `dest`.
    pub fn backup_to(&self, dest: &Path) -> Result<(), String> {
        self.repo()?
            .backup_with_files(dest, self.blobs.as_ref())
            .map_err(|e| e.to_string())
    }

    /// Replaces the database with the backup at `src`.
    ///
    /// When the database is open, a safety copy of it is written to the
    /// `safety-copies` folder first. When it could not be opened (for
    /// example because it is damaged), the backup is restored into a new
    /// file, and the unusable file is kept beside it with a `.damaged-…`
    /// suffix rather than deleted.
    pub fn restore_from(&self, src: &Path) -> Result<(), String> {
        let mut inner = self.lock()?;
        let dir = self
            .data_dir
            .clone()
            .ok_or("the data folder is unknown, so nothing can be restored")?;
        let text = |e: nimata_core::Error| e.to_string();

        if let Some(repo) = inner.repo.as_mut() {
            let copies = dir.join(SAFETY_COPIES);
            std::fs::create_dir_all(&copies).map_err(|e| e.to_string())?;
            let safety = copies.join(format!("before-restore-{}.sqlite3", timestamp_for_files()));
            repo.backup_to(&safety).map_err(text)?;
            return repo
                .restore_with_files(src, self.blobs.as_ref())
                .map_err(text);
        }

        // Restore into a separate file first, so a bad backup changes nothing.
        let live = dir.join(DATABASE_FILE);
        let restoring = dir.join("restoring.sqlite3");
        let _ = std::fs::remove_file(&restoring);
        {
            let mut fresh = SqliteRepository::open(&restoring).map_err(text)?;
            if let Err(e) = fresh.restore_with_files(src, self.blobs.as_ref()) {
                drop(fresh);
                let _ = std::fs::remove_file(&restoring);
                return Err(e.to_string());
            }
        }
        let suffix = format!("damaged-{}", timestamp_for_files());
        for extension in ["", "-wal", "-shm"] {
            let file = PathBuf::from(format!("{}{extension}", live.display()));
            if file.exists() {
                let kept = PathBuf::from(format!("{}.{suffix}{extension}", live.display()));
                std::fs::rename(&file, &kept).map_err(|e| e.to_string())?;
            }
        }
        std::fs::rename(&restoring, &live).map_err(|e| e.to_string())?;
        inner.repo = Some(SqliteRepository::open(&live).map_err(text)?);
        inner.error = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimata_core::{DiscussionFilter, Repository, Timestamp};
    use tempfile::TempDir;

    fn at(minutes: i64) -> Timestamp {
        Timestamp {
            at: UnixMillis(1_791_132_067_123 + minutes * 60_000),
            offset_minutes: 0,
        }
    }

    fn titles(state: &AppState) -> Vec<String> {
        state
            .repo()
            .unwrap()
            .list_discussions(DiscussionFilter::Active)
            .unwrap()
            .into_iter()
            .map(|s| s.title)
            .collect()
    }

    fn start(state: &AppState, title: &str) {
        let mut repo = state.repo().unwrap();
        let me = repo.local_user().unwrap();
        repo.start_discussion(title, me.id, "body", at(0)).unwrap();
    }

    #[test]
    fn creates_the_data_folder_and_database() {
        let dir = TempDir::new().unwrap();
        let data = dir.path().join("nested").join("org.nimata.app");
        let state = AppState::open_in(&data);
        assert!(state.repo().unwrap().local_user().is_ok());
        assert_eq!(state.database_path(), Some(data.join(DATABASE_FILE)));
        assert!(data.join(DATABASE_FILE).exists());
        assert_eq!(state.status().error, None);
    }

    #[test]
    fn an_unusable_folder_is_reported() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("not-a-folder");
        std::fs::write(&file, "x").unwrap();

        let error = AppState::open_in(&file.join("data")).repo().err().unwrap();
        assert!(error.starts_with("cannot create"), "{error}");
    }

    #[test]
    fn a_database_from_a_newer_version_is_reported_with_its_path() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(DATABASE_FILE);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        drop(conn);

        let state = AppState::open_in(dir.path());
        let error = state.repo().err().unwrap();
        assert!(error.contains("newer version of Nimata"), "{error}");
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert_eq!(state.status().error, Some(error));
    }

    #[test]
    fn restoring_keeps_a_safety_copy_of_what_it_replaced() {
        let dir = TempDir::new().unwrap();
        let state = AppState::open_in(dir.path());
        start(&state, "In the backup");
        let backup = dir.path().join("backup.sqlite3");
        state.backup_to(&backup).unwrap();
        start(&state, "After the backup");

        state.restore_from(&backup).unwrap();
        assert_eq!(titles(&state), vec!["In the backup"]);

        let copies: Vec<_> = std::fs::read_dir(dir.path().join(SAFETY_COPIES))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(copies.len(), 1, "{copies:?}");
        let safety = AppState::open_in(dir.path().join("check").as_path());
        safety.restore_from(&copies[0]).unwrap();
        assert_eq!(titles(&safety), vec!["After the backup", "In the backup"]);
    }

    #[test]
    fn a_damaged_database_can_be_replaced_by_a_backup() {
        let dir = TempDir::new().unwrap();
        let backup = dir.path().join("backup.sqlite3");
        let data = dir.path().join("data");
        {
            let state = AppState::open_in(&data);
            start(&state, "Saved");
            state.backup_to(&backup).unwrap();
        }
        let live = data.join(DATABASE_FILE);
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-wal", live.display())));
        let _ = std::fs::remove_file(PathBuf::from(format!("{}-shm", live.display())));
        std::fs::write(&live, "x".repeat(8192)).unwrap();

        let state = AppState::open_in(&data);
        assert!(state.status().error.is_some());

        // A bad backup is refused and the damaged file stays where it was.
        let not_a_backup = dir.path().join("notes.txt");
        std::fs::write(&not_a_backup, "hello").unwrap();
        assert!(state.restore_from(&not_a_backup).is_err());
        assert!(state.status().error.is_some());
        assert_eq!(std::fs::read(&live).unwrap(), "x".repeat(8192).into_bytes());

        state.restore_from(&backup).unwrap();
        assert_eq!(state.status().error, None);
        assert_eq!(titles(&state), vec!["Saved"]);
        let kept: Vec<_> = std::fs::read_dir(&data)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains(".damaged-"))
            .collect();
        assert_eq!(
            kept.len(),
            1,
            "the damaged file is kept, not deleted: {kept:?}"
        );
    }

    #[test]
    fn file_timestamps_contain_no_colons() {
        let stamp = timestamp_for_files();
        assert!(!stamp.contains(':'), "{stamp}");
        assert!(stamp.ends_with('Z'), "{stamp}");
    }
}
