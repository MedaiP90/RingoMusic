// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Disk cache and downloader of artwork images.
//!
//! The cache has no GTK types. The UI makes textures from the files, and the MPRIS
//! server points to them.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use std::{fs, io};

use reqwest::Client;
use sha2::{Digest, Sha256};
use tokio::sync::OnceCell;
use tokio::task::JoinError;
use tracing::{info, warn};

use crate::{Error, Result};

/// Size that the disk cache can have. `ArtworkStore::prune` removes the least recently used
/// files above this size.
pub const MAX_DISK_BYTES: u64 = 300 * 1024 * 1024;

/// Time that one download can take.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(20);

/// Returns the name of the cache file of `url`: the SHA-256 of the URL in
/// lowercase hexadecimal, and the extension `.jpg`.
#[must_use]
pub fn cache_file_name(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    let mut name = String::with_capacity(digest.len() * 2 + 4);
    for byte in &digest {
        write!(name, "{byte:02x}").expect("writing to a String cannot fail");
    }
    name.push_str(".jpg");
    name
}

/// A file of the disk cache.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedFile {
    /// Path of the file.
    pub path: PathBuf,
    /// Size in bytes.
    pub size: u64,
    /// Time of the last change.
    pub modified: SystemTime,
}

/// Selects the files to delete so that the rest fits in `limit` bytes. The
/// oldest files go first.
#[must_use]
pub fn files_to_prune(mut files: Vec<CachedFile>, limit: u64) -> Vec<PathBuf> {
    let mut total: u64 = files.iter().map(|file| file.size).sum();
    files.sort_by_key(|file| file.modified);
    let mut selected = Vec::new();
    for file in files {
        if total <= limit {
            break;
        }
        total -= file.size;
        selected.push(file.path);
    }
    selected
}

/// A download that more than one caller can wait for.
type SharedDownload = Arc<OnceCell<Option<PathBuf>>>;

#[derive(Debug)]
struct Inner {
    dir: PathBuf,
    client: Client,
    downloads: Mutex<HashMap<String, SharedDownload>>,
}

/// Artwork files on disk. A clone shares the same cache and the same downloads.
#[derive(Clone, Debug)]
pub struct ArtworkStore {
    inner: Arc<Inner>,
}

impl ArtworkStore {
    /// Makes a store for the directory `dir`. The directory must exist.
    ///
    /// # Errors
    ///
    /// Returns [`Error::HttpClient`] if the HTTP client cannot start.
    pub fn new(dir: PathBuf) -> Result<Self> {
        let client = Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .map_err(Error::HttpClient)?;
        Ok(Self {
            inner: Arc::new(Inner {
                dir,
                client,
                downloads: Mutex::default(),
            }),
        })
    }

    /// Returns the cache file of `url`, if it is on disk.
    #[must_use]
    pub fn cached_path(&self, url: &str) -> Option<PathBuf> {
        let path = self.inner.dir.join(cache_file_name(url));
        path.is_file().then_some(path)
    }

    /// Returns the `file://` URI of the cache file of `url`, if it is on disk.
    #[must_use]
    pub fn cached_uri(&self, url: &str) -> Option<String> {
        use gio::prelude::FileExt;

        let path = self.cached_path(url)?;
        Some(gio::File::for_path(path).uri().to_string())
    }

    /// Returns the cache file of `url`. It downloads the image first if the file
    /// is not on disk. Callers that ask for the same URL share one download.
    ///
    /// The call returns `None` if the download fails. The next call tries again.
    /// Run it on the Tokio runtime.
    pub async fn fetch(&self, url: &str) -> Option<PathBuf> {
        if let Some(path) = self.cached_path(url) {
            touch(&path);
            return Some(path);
        }
        let download = self.downloads().entry(url.to_owned()).or_default().clone();
        let path = download.get_or_init(|| self.download(url)).await.clone();
        self.downloads().remove(url);
        path
    }

    /// Deletes the oldest files if the cache is larger than [`MAX_DISK_BYTES`].
    /// The call blocks. Run it on a blocking thread.
    pub fn prune(&self) {
        match prune_dir(&self.inner.dir, MAX_DISK_BYTES) {
            Ok(0) => {}
            Ok(count) => info!(count, "removed old artwork files"),
            Err(error) => warn!(%error, "cannot prune the artwork cache"),
        }
    }

    fn downloads(&self) -> std::sync::MutexGuard<'_, HashMap<String, SharedDownload>> {
        self.inner
            .downloads
            .lock()
            .expect("internal error: a download thread panicked while it held the lock")
    }

    async fn download(&self, url: &str) -> Option<PathBuf> {
        match self.try_download(url).await {
            Ok(path) => Some(path),
            Err(error) => {
                warn!(%error, url, "cannot download artwork");
                None
            }
        }
    }

    async fn try_download(&self, url: &str) -> std::result::Result<PathBuf, DownloadError> {
        let response = self.inner.client.get(url).send().await?;
        let bytes = response.error_for_status()?.bytes().await?;
        let path = self.inner.dir.join(cache_file_name(url));
        let target = path.clone();
        tokio::task::spawn_blocking(move || write_atomically(&target, &bytes)).await??;
        Ok(path)
    }
}

/// Why a download failed.
#[derive(Debug, thiserror::Error)]
enum DownloadError {
    #[error("request failed")]
    Http(#[from] reqwest::Error),
    #[error("cannot write the file")]
    Write(#[from] io::Error),
    #[error("write task failed")]
    Task(#[from] JoinError),
}

/// Sets the modification time of `path` to now. [`ArtworkStore::prune`] uses the
/// time, so the files that the user shows stay in the cache. A failure only
/// makes the file look older.
fn touch(path: &Path) {
    let result = fs::File::options()
        .append(true)
        .open(path)
        .and_then(|file| file.set_modified(SystemTime::now()));
    if let Err(error) = result {
        warn!(%error, "cannot update the time of an artwork file");
    }
}

/// Writes `bytes` to a temporary file, and then renames it to `path`. A reader
/// never sees a half-written file.
fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("part");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)
}

/// Deletes the oldest files of `dir` until the rest fits in `limit` bytes.
/// Returns the number of deleted files.
fn prune_dir(dir: &Path, limit: u64) -> io::Result<usize> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            files.push(CachedFile {
                path: entry.path(),
                size: metadata.len(),
                modified: metadata.modified()?,
            });
        }
    }
    let selected = files_to_prune(files, limit);
    for path in &selected {
        fs::remove_file(path)?;
    }
    Ok(selected.len())
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    fn file(name: &str, size: u64, age_seconds: u64) -> CachedFile {
        CachedFile {
            path: PathBuf::from(name),
            size,
            modified: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 - age_seconds),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("ringo-artwork-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("test directory is created");
        dir
    }

    #[test]
    fn cache_file_name_is_the_sha256_of_the_url() {
        assert_eq!(
            cache_file_name("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad.jpg"
        );
    }

    #[test]
    fn cache_file_name_differs_for_different_urls() {
        assert_ne!(cache_file_name("a"), cache_file_name("b"));
    }

    #[test]
    fn files_to_prune_keeps_everything_below_the_limit() {
        let files = vec![file("a", 10, 1), file("b", 10, 2)];
        assert_eq!(files_to_prune(files, 20), Vec::<PathBuf>::new());
    }

    #[test]
    fn files_to_prune_removes_the_oldest_files_first() {
        let files = vec![
            file("new", 10, 1),
            file("oldest", 10, 3),
            file("old", 10, 2),
        ];
        let selected = files_to_prune(files, 15);
        assert_eq!(selected, [PathBuf::from("oldest"), PathBuf::from("old")]);
    }

    #[test]
    fn files_to_prune_stops_when_the_rest_fits() {
        let files = vec![file("new", 10, 1), file("old", 10, 2)];
        assert_eq!(files_to_prune(files, 10), [PathBuf::from("old")]);
    }

    #[test]
    fn files_to_prune_of_no_files_is_empty() {
        assert_eq!(files_to_prune(Vec::new(), 0), Vec::<PathBuf>::new());
    }

    #[test]
    fn write_atomically_leaves_only_the_target_file() {
        let dir = temp_dir("write");
        let target = dir.join("a.jpg");
        write_atomically(&target, b"data").expect("file is written");
        assert_eq!(fs::read(&target).expect("file exists"), b"data");
        assert_eq!(fs::read_dir(&dir).expect("directory exists").count(), 1);
        fs::remove_dir_all(dir).expect("test directory is removed");
    }

    #[test]
    fn cached_path_exists_only_for_a_file_on_disk() {
        let dir = temp_dir("cached");
        let store = ArtworkStore::new(dir.clone()).expect("client starts");
        assert_eq!(store.cached_path("https://x/a"), None);
        let path = dir.join(cache_file_name("https://x/a"));
        fs::write(&path, b"data").expect("file is written");
        assert_eq!(store.cached_path("https://x/a"), Some(path));
        fs::remove_dir_all(dir).expect("test directory is removed");
    }

    #[test]
    fn touch_sets_the_modification_time_to_now() {
        let dir = temp_dir("touch");
        let path = dir.join("a.jpg");
        fs::write(&path, b"data").expect("file is written");
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        fs::File::options()
            .append(true)
            .open(&path)
            .and_then(|file| file.set_modified(old))
            .expect("time is set");
        touch(&path);
        let modified = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .expect("time is known");
        assert!(modified > old, "time {modified:?} is not after {old:?}");
        fs::remove_dir_all(dir).expect("test directory is removed");
    }

    #[test]
    fn prune_dir_deletes_files_above_the_limit() {
        let dir = temp_dir("prune");
        fs::write(dir.join("a.jpg"), [0_u8; 10]).expect("file is written");
        fs::write(dir.join("b.jpg"), [0_u8; 10]).expect("file is written");
        assert_eq!(prune_dir(&dir, 10).expect("directory is pruned"), 1);
        assert_eq!(fs::read_dir(&dir).expect("directory exists").count(), 1);
        fs::remove_dir_all(dir).expect("test directory is removed");
    }
}
