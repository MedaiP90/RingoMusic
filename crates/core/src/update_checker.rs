// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 RingoMusic contributors

//! Check for a newer release on GitHub.
//!
//! The check sends no personal data. It fails quietly: a failure is only logged.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use semver::Version;
use serde::Deserialize;
use tracing::debug;

use crate::{Error, Result};

/// Address of the latest release. GitHub leaves out pre-releases here.
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/MedaiP90/RingoMusic/releases/latest";

/// Marker file, relative to the install prefix. A package that updates the
/// application itself (for example the AUR package) installs it.
const MARKER_FILE: &str = "share/ringomusic/no-update-check";

/// Time that the request can take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Minimum time between two checks, in seconds.
const CHECK_INTERVAL_SECS: i64 = 24 * 60 * 60;

/// Whether the package turned the check off with the marker file.
#[must_use]
pub fn disabled_by_package(install_prefix: &Path) -> bool {
    install_prefix.join(MARKER_FILE).exists()
}

/// Whether a check must run now.
///
/// The check is off when the user turned it off, or when the package did. It also
/// waits 24 hours after the last check. A `last_check` in the future (the clock
/// moved back) does not block the check.
#[must_use]
pub fn should_check(enabled: bool, install_prefix: &Path, last_check: i64, now: i64) -> bool {
    enabled
        && !disabled_by_package(install_prefix)
        && (last_check > now || now - last_check >= CHECK_INTERVAL_SECS)
}

/// Current Unix time, in seconds.
#[must_use]
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

/// A published release.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Release {
    /// Version of the release.
    pub version: Version,
    /// Address of the release page.
    pub url: String,
}

impl Release {
    /// Reads a release from the JSON that the GitHub API gives.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UpdateResponse`] if the JSON has no `tag_name` and
    /// `html_url`, and [`Error::InvalidVersion`] if the tag is not a version.
    pub fn from_json(json: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct Response {
            tag_name: String,
            html_url: String,
        }

        let response: Response = serde_json::from_slice(json).map_err(Error::UpdateResponse)?;
        let version = parse_version(&response.tag_name)?;
        Ok(Self {
            version,
            url: response.html_url,
        })
    }
}

/// Parses a version. A leading `v`, as in a Git tag, is allowed.
fn parse_version(text: &str) -> Result<Version> {
    let digits = text.strip_prefix('v').unwrap_or(text);
    Version::parse(digits).map_err(|source| Error::InvalidVersion {
        version: text.to_owned(),
        source,
    })
}

/// Reads the latest release from GitHub and compares it with the running version.
#[derive(Clone, Debug)]
pub struct UpdateChecker {
    client: Client,
    current: Version,
}

impl UpdateChecker {
    /// Makes a checker for the running version `current`, for example `0.9.0`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidVersion`] if `current` is not a version, and
    /// [`Error::HttpClient`] if the HTTP client cannot start.
    pub fn new(current: &str) -> Result<Self> {
        let current = parse_version(current)?;
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(format!("RingoMusic/{current}"))
            .build()
            .map_err(Error::HttpClient)?;
        Ok(Self { client, current })
    }

    /// Returns the latest release if it is newer than the running version.
    ///
    /// A failure gives `None`. It is logged at `debug` level. Run it on the Tokio
    /// runtime.
    pub async fn newer_release(&self) -> Option<Release> {
        match self.fetch_latest().await {
            Ok(release) => self.is_newer(&release).then_some(release),
            Err(error) => {
                debug!(?error, "update check failed");
                None
            }
        }
    }

    /// Whether `release` is newer than the running version.
    #[must_use]
    pub fn is_newer(&self, release: &Release) -> bool {
        release.version > self.current
    }

    async fn fetch_latest(&self) -> Result<Release> {
        let body = self
            .client
            .get(LATEST_RELEASE_URL)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(Error::UpdateRequest)?
            .bytes()
            .await
            .map_err(Error::UpdateRequest)?;
        Release::from_json(&body)
    }
}

#[cfg(test)]
mod tests;
