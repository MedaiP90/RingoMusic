// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

use std::fs::{self, DirBuilder, Permissions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::Path;

use tokio::net::UnixListener;

/// Mode of the socket directory. Only the owner has access.
const DIRECTORY_MODE: u32 = 0o700;

/// Creates the private socket directory, removes a stale socket, and binds.
///
/// Must run inside a Tokio runtime.
pub(crate) fn bind(path: &Path) -> io::Result<UnixListener> {
    if let Some(directory) = path.parent() {
        DirBuilder::new()
            .recursive(true)
            .mode(DIRECTORY_MODE)
            .create(directory)?;
        // The directory can exist already with another mode.
        fs::set_permissions(directory, Permissions::from_mode(DIRECTORY_MODE))?;
    }
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    UnixListener::bind(path)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process;

    use super::*;

    #[tokio::test]
    async fn bind_creates_private_directory_and_replaces_stale_socket() {
        let root: PathBuf = std::env::temp_dir().join(format!("ringo-socket-{}", process::id()));
        let socket = root.join("ringomusic/engine.sock");

        drop(bind(&socket).expect("first bind works"));
        // The socket file stays after the listener drops. This is a stale socket.
        assert!(socket.exists());
        let _listener = bind(&socket).expect("bind replaces the stale socket");

        let mode = fs::metadata(socket.parent().expect("socket has a parent"))
            .expect("directory exists")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, DIRECTORY_MODE);
        fs::remove_dir_all(root).expect("test directory is removed");
    }
}
