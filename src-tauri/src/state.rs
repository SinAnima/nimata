use std::path::PathBuf;
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
        let dir = match app.path().app_data_dir() {
            Ok(dir) => dir,
            Err(e) => return Self::failed(format!("cannot locate the data folder: {e}"), None),
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
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
