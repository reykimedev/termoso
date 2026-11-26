//! In-app updates. Manifests and packages are minisign-verified against the
//! public key baked into the binary; the feed URL is user-configurable so a
//! self-hosted server can serve it. Nothing is fetched unless the user asks
//! for it or explicitly enables the startup check.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};
use url::Url;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

pub const UPDATE_EVENT: &str = "update";

/// Release channel, set at build time: unset for stable releases, `dev` for
/// the rolling builds from `main` (`.github/workflows/devel.yml`).
/// Each channel has its own feed and its own signing key.
pub const CHANNEL: Option<&str> = option_env!("LABTERM_CHANNEL");

/// Release feed used when `Settings::update_url` is empty. Dev builds bake in
/// the dev feed, so they never offer a stable build and stable never a dev one.
pub const DEFAULT_FEED: &str = match option_env!("LABTERM_UPDATE_FEED") {
    Some(feed) => feed,
    None => "https://github.com/reykimedev/termoso/releases/latest/download/latest.json",
};

/// Upstream Labterm's updater key. A binary that still trusts it would accept
/// Labterm's releases as updates and replace Labterm with Labterm, so the
/// updater stays off in such a build.
const UPSTREAM_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IENDMjE2QzA2NTRBQUEzQjgKUldTNG82cFVCbXdoelBNRER5bFBWcy82cWdWYUhwdk5sN3MzUXF5WU5YRnY4TUl5S0NxMHJUY2YK";

/// Whether an updater public key from the config is one Labterm may trust.
fn trusted_pubkey(pubkey: Option<&str>) -> bool {
    matches!(pubkey.map(str::trim), Some(k) if !k.is_empty() && k != UPSTREAM_PUBKEY)
}

/// In-app updates work only when the binary carries Labterm's own key.
pub fn enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    let pubkey = app
        .config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str());
    trusted_pubkey(pubkey)
}

/// Pending update found by the last check; kept in Rust so the webview only
/// ever sees metadata.
#[derive(Default)]
pub struct UpdateHub {
    pending: Mutex<Option<Update>>,
    busy: Mutex<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub download_url: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateEvent {
    Available {
        info: UpdateInfo,
    },
    Progress {
        downloaded: u64,
        total: Option<u64>,
    },
    /// Download complete; checking the signature against the built-in key.
    Verifying,
    /// Signature verified; the platform installer is running.
    Installing,
    Installed {
        version: String,
    },
    Failed {
        message: String,
    },
}

fn updater_err(e: tauri_plugin_updater::Error) -> DesktopError {
    DesktopError::new("update", describe(&e))
}

/// What went wrong, in words a user can act on. The plugin's own text
/// ("Could not fetch a valid release JSON from the remote", a bare minisign
/// error) is kept for the log.
fn describe(e: &tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error as E;
    match e {
        E::Reqwest(r) if r.is_timeout() => {
            "The update server did not answer in time. Check your connection and try again.".into()
        }
        E::Reqwest(r) if r.is_connect() => {
            "Could not connect to the update server. Check your connection, or the release feed in Settings → Updates.".into()
        }
        E::Reqwest(r) => format!("Network error while talking to the update server: {r}"),
        E::Network(msg) => format!("The update server refused the request ({msg})."),
        E::ReleaseNotFound | E::Serialization(_) | E::Semver(_) => {
            "The release feed did not return a valid latest.json. Check the feed URL in Settings → Updates.".into()
        }
        E::TargetNotFound(t) => format!("The latest release has no build for this platform ({t})."),
        E::TargetsNotFound(ts) => format!(
            "The latest release has no build for this platform ({}).",
            ts.join(", ")
        ),
        E::Minisign(_)
        | E::Base64(_)
        | E::SignatureUtf8(_)
        | E::SignedVersionMismatch { .. }
        | E::MissingSignedVersion => {
            "The update is not signed with the key built into this app, so it was not installed. Nothing on this computer was changed.".into()
        }
        E::InsecureTransportProtocol => "The release feed must use https.".into(),
        E::AuthenticationFailed => {
            "Installing needs administrator rights, and the request was cancelled or failed.".into()
        }
        E::DebInstallFailed | E::PackageInstallFailed => {
            "The system package installer failed. Install the downloaded package by hand, or try again.".into()
        }
        E::Io(io) => format!("Could not write the update to disk: {io}"),
        other => other.to_string(),
    }
}

/// Bytes per second as "3.1 MB/s", for the log.
fn rate(bytes: u64, secs: f64) -> String {
    if secs <= 0.0 {
        return "-".into();
    }
    format!("{:.1} MB/s", bytes as f64 / secs / 1_000_000.0)
}

fn info(u: &Update) -> UpdateInfo {
    UpdateInfo {
        current_version: u.current_version.clone(),
        version: u.version.clone(),
        notes: u.body.clone(),
        published_at: u.date.and_then(|d| {
            d.format(&time::format_description::well_known::Rfc3339)
                .ok()
        }),
        download_url: u.download_url.to_string(),
        target: u.target.clone(),
    }
}

/// Resolve the feed URL: user setting, else the project default. Only
/// `https://` is accepted; the plugin enforces the same in release builds.
pub fn feed_url(raw: &str) -> Result<Url> {
    let raw = raw.trim();
    let url = Url::parse(if raw.is_empty() { DEFAULT_FEED } else { raw })
        .map_err(|e| DesktopError::invalid(format!("updateUrl: {e}")))?;
    if url.scheme() != "https" {
        return Err(DesktopError::invalid("updateUrl must use https"));
    }
    Ok(url)
}

fn emit<R: Runtime>(app: &AppHandle<R>, ev: UpdateEvent) {
    let _ = app.emit(UPDATE_EVENT, ev);
}

/// Contact the configured feed once and remember the result.
pub async fn check<R: Runtime>(app: &AppHandle<R>) -> Result<Option<UpdateInfo>> {
    if !enabled(app) {
        return Err(DesktopError::new(
            "update",
            "in-app updates are turned off in this build",
        ));
    }
    let settings = app.state::<AppState>().settings()?;
    let url = feed_url(&settings.update_url)?;
    let current = app.package_info().version.to_string();
    tracing::info!(feed = %url, %current, "update check");
    let updater = app
        .updater_builder()
        .endpoints(vec![url.clone()])
        .map_err(updater_err)?
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(updater_err)?;
    let found = updater.check().await.map_err(|e| {
        tracing::warn!(feed = %url, error = %e, "update check failed");
        updater_err(e)
    })?;
    match &found {
        Some(u) => tracing::info!(
            version = %u.version,
            target = %u.target,
            url = %u.download_url,
            "update found"
        ),
        None => tracing::info!(%current, "no update: already on the latest version"),
    }
    let hub = app.state::<UpdateHub>();
    let mut pending = hub.pending.lock().map_err(|_| poisoned())?;
    let out = found.as_ref().map(info);
    *pending = found;
    Ok(out)
}

/// Download, verify and install the update found by the last [`check`].
/// Emits progress; the caller decides when to relaunch.
pub async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<UpdateInfo> {
    let hub = app.state::<UpdateHub>();
    let update = {
        let mut busy = hub.busy.lock().map_err(|_| poisoned())?;
        if *busy {
            return Err(DesktopError::new("busy", "an update is already installing"));
        }
        let pending = hub.pending.lock().map_err(|_| poisoned())?;
        let Some(u) = pending.clone() else {
            return Err(DesktopError::not_found(
                "no pending update; check for updates first",
            ));
        };
        *busy = true;
        u
    };

    let meta = info(&update);
    tracing::info!(version = %meta.version, url = %meta.download_url, "update download started");
    let started = Instant::now();
    let progress_app = app.clone();
    let verify_app = app.clone();
    let mut downloaded: u64 = 0;
    let mut size: Option<u64> = None;
    let mut last_emit: Option<Instant> = None;
    let downloaded_bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if size.is_none() && total.is_some() {
                    size = total;
                    tracing::info!(bytes = total, "update download size");
                }
                // A few events a second are plenty for the UI; one per chunk floods the IPC.
                let done = total.is_some_and(|t| downloaded >= t);
                if done || last_emit.is_none_or(|t| t.elapsed() >= Duration::from_millis(200)) {
                    last_emit = Some(Instant::now());
                    emit(&progress_app, UpdateEvent::Progress { downloaded, total });
                }
            },
            || emit(&verify_app, UpdateEvent::Verifying),
        )
        .await;
    let result = downloaded_bytes.and_then(|bytes| {
        let secs = started.elapsed().as_secs_f64();
        tracing::info!(
            bytes = bytes.len(),
            secs = format!("{secs:.1}"),
            speed = rate(bytes.len() as u64, secs),
            "update downloaded, signature verified"
        );
        emit(app, UpdateEvent::Installing);
        update.install(&bytes)
    });

    if let Ok(mut busy) = hub.busy.lock() {
        *busy = false;
    }
    match result {
        Ok(()) => {
            tracing::info!(version = %meta.version, "update installed; restart to finish");
            if let Ok(mut pending) = hub.pending.lock() {
                *pending = None;
            }
            emit(
                app,
                UpdateEvent::Installed {
                    version: meta.version.clone(),
                },
            );
            Ok(meta)
        }
        Err(e) => {
            tracing::error!(version = %meta.version, error = %e, "update failed");
            let err = updater_err(e);
            emit(
                app,
                UpdateEvent::Failed {
                    message: err.message.clone(),
                },
            );
            Err(err)
        }
    }
}

/// Startup hook: only runs when the user opted in via `updateCheck = "startup"`.
pub async fn check_on_startup<R: Runtime>(app: &AppHandle<R>) {
    match check(app).await {
        Ok(Some(i)) => {
            tracing::info!(version = %i.version, "update available");
            emit(app, UpdateEvent::Available { info: i });
        }
        Ok(None) => tracing::debug!("no update available"),
        Err(e) => tracing::warn!("update check failed: {e}"),
    }
}

/// How often `updateCheck` asks for a background check after the startup one;
/// `None` for the modes that never poll (`manual`, `startup`).
pub fn poll_interval(setting: &str) -> Option<Duration> {
    let hours = match setting {
        "every_6h" => 6,
        "daily" => 24,
        "weekly" => 24 * 7,
        _ => return None,
    };
    Some(Duration::from_secs(hours * 3600))
}

/// Background checks for the interval modes. The setting is re-read every
/// minute, so changing it (or switching back to "manual") takes effect without
/// a restart. A version is announced once, so a dismissed banner stays gone.
pub async fn scheduler<R: Runtime>(app: AppHandle<R>) {
    let mut last = Instant::now();
    let mut announced: Option<String> = None;
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let Ok(settings) = app.state::<AppState>().settings() else {
            continue;
        };
        let Some(every) = poll_interval(&settings.update_check) else {
            last = Instant::now();
            continue;
        };
        if last.elapsed() < every {
            continue;
        }
        last = Instant::now();
        match check(&app).await {
            Ok(Some(i)) => {
                if announced.as_deref() != Some(i.version.as_str()) {
                    tracing::info!(version = %i.version, "update available");
                    announced = Some(i.version.clone());
                    emit(&app, UpdateEvent::Available { info: i });
                }
            }
            Ok(None) => tracing::debug!("no update available"),
            Err(e) => tracing::warn!("scheduled update check failed: {e}"),
        }
    }
}

fn poisoned() -> DesktopError {
    DesktopError::new("internal", "updater state poisoned")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_setting_falls_back_to_default_feed() {
        assert_eq!(feed_url("").unwrap().as_str(), DEFAULT_FEED);
        assert_eq!(feed_url("   ").unwrap().as_str(), DEFAULT_FEED);
    }

    #[test]
    fn custom_https_feed_is_kept() {
        let u =
            feed_url("https://updates.example.org/labterm/{{target}}/{{current_version}}").unwrap();
        assert_eq!(u.host_str(), Some("updates.example.org"));
    }

    #[test]
    fn upstream_or_missing_key_keeps_the_updater_off() {
        assert!(!trusted_pubkey(None));
        assert!(!trusted_pubkey(Some("")));
        assert!(!trusted_pubkey(Some(UPSTREAM_PUBKEY)));
        assert!(trusted_pubkey(Some(
            "dW50cnVzdGVkIGNvbW1lbnQ6IG90aGVyIGtleQ=="
        )));
    }

    #[test]
    fn default_feed_is_labterms_own() {
        assert!(!DEFAULT_FEED.contains("labterm/labterm"));
        assert!(DEFAULT_FEED.starts_with("https://"));
    }

    #[test]
    fn readable_errors() {
        use tauri_plugin_updater::Error as E;
        assert!(describe(&E::TargetNotFound("linux-x86_64".into())).contains("linux-x86_64"));
        assert!(describe(&E::ReleaseNotFound).contains("latest.json"));
        assert!(describe(&E::MissingSignedVersion).contains("not signed with the key"));
        assert_eq!(rate(3_000_000, 2.0), "1.5 MB/s");
        assert_eq!(rate(1, 0.0), "-");
    }

    #[test]
    fn poll_intervals() {
        assert_eq!(poll_interval("manual"), None);
        assert_eq!(poll_interval("startup"), None);
        assert_eq!(
            poll_interval("every_6h"),
            Some(Duration::from_secs(6 * 3600))
        );
        assert_eq!(poll_interval("daily"), Some(Duration::from_secs(24 * 3600)));
        assert_eq!(
            poll_interval("weekly"),
            Some(Duration::from_secs(7 * 24 * 3600))
        );
    }

    #[test]
    fn plain_http_is_rejected() {
        assert!(feed_url("http://updates.example.org/latest.json").is_err());
        assert!(feed_url("ftp://x").is_err());
        assert!(feed_url("not a url").is_err());
    }
}
