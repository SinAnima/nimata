use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use nimata_core::SqliteRepository;
use tauri::{AppHandle, Manager, Runtime};

pub const DATABASE_FILE: &str = "nimata.sqlite3";

pub struct AppState {
    /// The open database, or the reason it could not be opened. A failure is
    /// reported to the UI instead of crashing, so the user can see what
    /// happened.
    repo: Result<Mutex<SqliteRepository>, String>,
    pub database_path: Option<PathBuf>,
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
            Ok(repo) => Self {
                repo: Ok(Mutex::new(repo)),
                database_path: Some(path),
            },
            Err(e) => {
                let message = format!("cannot open {}: {e}", path.display());
                Self::failed(message, Some(path))
            }
        }
    }

    #[cfg(test)]
    pub fn with_repo(repo: SqliteRepository) -> Self {
        Self {
            repo: Ok(Mutex::new(repo)),
            database_path: None,
        }
    }

    fn failed(message: String, database_path: Option<PathBuf>) -> Self {
        Self {
            repo: Err(message),
            database_path,
        }
    }

    pub fn repo(&self) -> Result<MutexGuard<'_, SqliteRepository>, String> {
        let mutex = self.repo.as_ref().map_err(Clone::clone)?;
        mutex
            .lock()
            .map_err(|_| "the database is unavailable after an earlier failure".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimata_core::Repository;
    use tempfile::TempDir;

    #[test]
    fn creates_the_data_folder_and_database() {
        let dir = TempDir::new().unwrap();
        let data = dir.path().join("nested").join("org.nimata.app");
        let state = AppState::open_in(&data);
        assert!(state.repo().unwrap().local_user().is_ok());
        assert_eq!(state.database_path, Some(data.join(DATABASE_FILE)));
        assert!(data.join(DATABASE_FILE).exists());
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
        rusqlite_set_user_version(&path, 99);

        let state = AppState::open_in(dir.path());
        let error = state.repo().err().unwrap();
        assert!(error.contains("newer version of Nimata"), "{error}");
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert_eq!(state.database_path, Some(path));
    }

    fn rusqlite_set_user_version(path: &Path, version: i64) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.pragma_update(None, "user_version", version).unwrap();
    }
}
