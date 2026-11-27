//! Master password and App Lock.
//!
//! The vault is always encrypted with a random device key; the password only
//! wraps that key on disk (`master.pw`, see `labterm_core::secrets`). While the
//! vault is locked the `Store` — and with it the key — is gone from memory, so
//! every vault command fails with `locked` until the password is entered.
//!
//! Unlock factors (Windows Hello for now, see `labterm_core::secrets::factors`)
//! are more wrappers of the same key; the password is then an optional
//! fallback.

use labterm_core::secrets::{MIN_PASSWORD_CHARS, MasterKeySource, factors::FactorInfo};
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use zeroize::Zeroizing;

use crate::error::{DesktopError, Result};
use crate::state::AppState;

pub const VAULT_EVENT: &str = "vault";

/// How often the inactivity watcher looks at the clock.
const WATCH_TICK: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub locked: bool,
    /// Starts locked: a master password or an unlock factor holds the key.
    pub protected: bool,
    pub password_protected: bool,
    /// Enrolled unlock factors (no secrets).
    pub factors: Vec<FactorInfo>,
    /// Windows Hello can be enrolled on this machine.
    pub hello_available: bool,
    pub master_source: MasterKeySource,
    pub min_password_chars: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VaultEvent {
    Locked,
    Unlocked,
}

pub fn status(state: &AppState) -> VaultStatus {
    VaultStatus {
        locked: state.is_locked(),
        protected: state.protected(),
        password_protected: state.password_protected(),
        factors: state.factors(),
        hello_available: hello_available(),
        master_source: state.master_source(),
        min_password_chars: MIN_PASSWORD_CHARS,
    }
}

/// Asking Windows can take a moment; the answer does not change while the
/// app runs, so ask once.
fn hello_available() -> bool {
    static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAILABLE.get_or_init(crate::hello::available)
}

/// Tear down everything that holds vault data or a clone of the store, then
/// drop the store itself. Idempotent.
pub async fn lock<R: Runtime>(app: &AppHandle<R>) -> Result<VaultStatus> {
    let state = app.state::<AppState>();
    if !state.protected() {
        return Err(DesktopError::invalid(
            "set a master password or Windows Hello before locking the vault",
        ));
    }
    if state.is_locked() {
        return Ok(status(&state));
    }
    if state
        .lock
        .locking
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err(DesktopError::invalid("the vault is already locking"));
    }
    shutdown(app).await;
    state.close_store();
    state.lock.locking.store(false, Ordering::SeqCst);
    tracing::info!("vault locked");
    let _ = app.emit(VAULT_EVENT, VaultEvent::Locked);
    Ok(status(&state))
}

async fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    crate::multiplayer::stop_all(app).await;
    crate::sessions::close_all(app).await;
    crate::sftp::close_all(app).await;
    crate::forwarding::stop_all(app).await;
    crate::agent_server::stop(app).await;
    crate::vpn::stop_all().await;
    app.state::<AppState>().edits.close_all();
    crate::account::suspend(app).await;
}

/// Open the store with `password` and resume background work.
pub async fn unlock(app: &AppHandle, password: Zeroizing<String>) -> Result<VaultStatus> {
    let state = app.state::<AppState>();
    if state.lock.locking.load(Ordering::SeqCst) {
        return Err(DesktopError::invalid("the vault is still locking"));
    }
    if !state.is_locked() {
        return Ok(status(&state));
    }
    if !state.password_protected() {
        return Err(DesktopError::invalid("the vault has no master password"));
    }
    state.unlock_with_password(&password)?;
    drop(password);
    unlocked(app)
}

fn unlocked(app: &AppHandle) -> Result<VaultStatus> {
    let state = app.state::<AppState>();
    tracing::info!(source = ?state.master_source(), "vault unlocked");
    let _ = app.emit(VAULT_EVENT, VaultEvent::Unlocked);
    tauri::async_runtime::spawn(crate::startup(app.clone()));
    Ok(status(&state))
}

/// Open the store with Windows Hello: prompts Hello for each enrolled Hello
/// factor in turn until one signs (normally the first). A cancel stops.
pub async fn unlock_hello(app: &AppHandle) -> Result<VaultStatus> {
    let state = app.state::<AppState>();
    if state.lock.locking.load(Ordering::SeqCst) {
        return Err(DesktopError::invalid("the vault is still locking"));
    }
    if !state.is_locked() {
        return Ok(status(&state));
    }
    let challenges = state.hello_challenges()?;
    if challenges.is_empty() {
        return Err(DesktopError::invalid(
            "Windows Hello is not set up for Labterm",
        ));
    }
    let mut last = None;
    for c in challenges {
        let name = String::from_utf8_lossy(&c.credential).into_owned();
        let salt = c.salt.clone();
        let signed = tauri::async_runtime::spawn_blocking(move || crate::hello::sign(&name, &salt))
            .await
            .map_err(|e| DesktopError::invalid(e.to_string()))?;
        match signed {
            Ok(secret) => {
                let secret = Zeroizing::new(secret);
                state.unlock_with_factor(&c.id, &secret)?;
                return unlocked(app);
            }
            Err(e) if e.kind == "cancelled" => return Err(e),
            Err(e) => {
                tracing::warn!(factor = %c.id, "Windows Hello unlock failed: {e}");
                last = Some(e);
            }
        }
    }
    Err(last.unwrap_or_else(|| DesktopError::invalid("Windows Hello failed")))
}

/// Relock after `lockAfterMinutes` without user input. One instance per
/// process; cheap enough to run for the app's lifetime.
pub async fn inactivity_watcher(app: AppHandle) {
    loop {
        tokio::time::sleep(WATCH_TICK).await;
        let state = app.state::<AppState>();
        if state.is_locked() || !state.protected() {
            continue;
        }
        let Ok(settings) = state.settings() else {
            continue;
        };
        if state
            .lock
            .idle_expired(settings.lock_after_minutes, Instant::now())
        {
            tracing::info!(
                minutes = settings.lock_after_minutes,
                "locking vault after inactivity"
            );
            if let Err(e) = lock(&app).await {
                tracing::warn!("inactivity lock failed: {e}");
            }
        }
    }
}

// ───────────────────────────── commands ─────────────────────────────

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    status(&state)
}

#[tauri::command]
pub async fn vault_unlock(app: AppHandle, password: String) -> Result<VaultStatus> {
    unlock(&app, Zeroizing::new(password)).await
}

#[tauri::command]
pub async fn vault_lock(app: AppHandle) -> Result<VaultStatus> {
    lock(&app).await
}

/// The webview saw keyboard / pointer input; resets the inactivity timer.
#[tauri::command]
pub fn vault_activity(state: State<'_, AppState>) {
    state.lock.touch();
}

/// Enable a master password or change the current one (`current` required
/// when one is set). The device key is only re-wrapped; the database itself
/// is untouched.
#[tauri::command]
pub async fn master_password_set(
    app: AppHandle,
    current: Option<String>,
    password: String,
) -> Result<VaultStatus> {
    let current = current.map(Zeroizing::new);
    let password = Zeroizing::new(password);
    let handle = app.clone();
    // May wait for a Windows Hello confirmation: keep it off the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        handle
            .state::<AppState>()
            .set_master_password(current.as_deref().map(String::as_str), &password)
    })
    .await
    .map_err(|e| DesktopError::invalid(e.to_string()))??;
    tracing::info!("master password set");
    Ok(status(&app.state::<AppState>()))
}

#[tauri::command]
pub async fn vault_unlock_hello(app: AppHandle) -> Result<VaultStatus> {
    unlock_hello(&app).await
}

/// Enroll Windows Hello. `password` proves the owner when a master password
/// is set; otherwise an already enrolled Hello factor is asked first.
#[tauri::command]
pub async fn unlock_hello_add(
    app: AppHandle,
    password: Option<String>,
    name: String,
) -> Result<VaultStatus> {
    let password = password.map(Zeroizing::new);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<AppState>();
        state.add_hello_factor(password.as_deref().map(String::as_str), &name)
    })
    .await
    .map_err(|e| DesktopError::invalid(e.to_string()))??;
    tracing::info!("Windows Hello unlock enrolled");
    Ok(status(&app.state::<AppState>()))
}

/// Remove an unlock factor; the last way in turns protection off instead
/// (after confirming with it).
#[tauri::command]
pub async fn unlock_factor_remove(app: AppHandle, id: String) -> Result<VaultStatus> {
    let handle = app.clone();
    let source =
        tauri::async_runtime::spawn_blocking(move || handle.state::<AppState>().remove_factor(&id))
            .await
            .map_err(|e| DesktopError::invalid(e.to_string()))??;
    tracing::info!(source = ?source, "unlock method removed");
    Ok(status(&app.state::<AppState>()))
}

#[tauri::command]
pub fn unlock_factor_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<VaultStatus> {
    state.rename_factor(&id, &name)?;
    Ok(status(&state))
}

/// Remove the master password after proving it; the key goes back to the OS
/// keychain (or the owner-only file where there is none).
#[tauri::command]
pub async fn master_password_remove(
    state: State<'_, AppState>,
    current: String,
) -> Result<VaultStatus> {
    let current = Zeroizing::new(current);
    let source = state.remove_master_password(&current)?;
    tracing::info!(source = ?source, "master password removed");
    Ok(status(&state))
}
