//! Windows Hello as an unlock factor.
//!
//! `KeyCredentialManager` keeps a TPM-backed RSA key per enrolled factor that
//! only signs after Windows Hello (face, fingerprint or Hello PIN) checked the
//! user. Its PKCS#1 v1.5 signature over the factor's stored random challenge
//! is deterministic, so it serves as the secret the vault key is wrapped
//! under (`labterm_core::secrets::factors`); the key itself never leaves the
//! TPM. Every call blocks while the Hello dialog is up: run it off the async
//! runtime.

use crate::error::{DesktopError, Result};

/// Prefix of the `KeyCredentialManager` key names, followed by a random id.
pub const KEY_NAME_PREFIX: &str = "Labterm unlock ";

/// Whether Windows Hello is set up and usable on this machine.
pub fn available() -> bool {
    imp::available()
}

/// Create a new Hello key named `name` (the user confirms with Hello) and
/// return its signature over `challenge`.
pub fn enroll(name: &str, challenge: &[u8]) -> Result<Vec<u8>> {
    imp::sign(name, challenge, true)
}

/// Sign `challenge` with the existing Hello key `name` (prompts Hello).
pub fn sign(name: &str, challenge: &[u8]) -> Result<Vec<u8>> {
    imp::sign(name, challenge, false)
}

/// Delete the Hello key `name`. Missing keys are fine.
pub fn delete(name: &str) {
    imp::delete(name);
}

#[cfg_attr(not(windows), allow(dead_code))]
fn cancelled() -> DesktopError {
    DesktopError::new("cancelled", "Windows Hello was cancelled")
}

#[cfg(windows)]
mod imp {
    use std::time::{Duration, Instant};

    use windows::Security::Credentials::{
        KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
    };
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow};
    use windows::core::{HSTRING, w};

    use super::cancelled;
    use crate::error::{DesktopError, Result};

    fn hello_err(e: windows::core::Error) -> DesktopError {
        DesktopError::new("hello", format!("Windows Hello failed: {}", e.message()))
    }

    fn status_err(status: KeyCredentialStatus) -> DesktopError {
        match status {
            KeyCredentialStatus::UserCanceled => cancelled(),
            KeyCredentialStatus::NotFound => DesktopError::new(
                "hello_key_missing",
                "the Windows Hello key for Labterm is gone; unlock another way and set it up again",
            ),
            KeyCredentialStatus::SecurityDeviceLocked => DesktopError::new(
                "hello",
                "Windows Hello is locked after too many attempts; try again later",
            ),
            KeyCredentialStatus::UserPrefersPassword => DesktopError::new(
                "hello",
                "Windows Hello was declined; use another way to unlock",
            ),
            _ => DesktopError::new("hello", "Windows Hello is not available right now"),
        }
    }

    pub fn available() -> bool {
        KeyCredentialManager::IsSupportedAsync()
            .and_then(|op| op.join())
            .unwrap_or(false)
    }

    pub fn sign(name: &str, challenge: &[u8], create: bool) -> Result<Vec<u8>> {
        let name = HSTRING::from(name);
        raise_dialog();
        let retrieved = if create {
            KeyCredentialManager::RequestCreateAsync(
                &name,
                KeyCredentialCreationOption::FailIfExists,
            )
        } else {
            KeyCredentialManager::OpenAsync(&name)
        }
        .and_then(|op| op.join())
        .map_err(hello_err)?;
        let status = retrieved.Status().map_err(hello_err)?;
        if status != KeyCredentialStatus::Success {
            return Err(status_err(status));
        }
        let credential = retrieved.Credential().map_err(hello_err)?;
        let data = CryptographicBuffer::CreateFromByteArray(challenge).map_err(hello_err)?;
        if !create {
            raise_dialog();
        }
        let signed = credential
            .RequestSignAsync(&data)
            .and_then(|op| op.join())
            .map_err(hello_err)?;
        let status = signed.Status().map_err(hello_err)?;
        if status != KeyCredentialStatus::Success {
            return Err(status_err(status));
        }
        let buffer = signed.Result().map_err(hello_err)?;
        let mut out = windows::core::Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(&buffer, &mut out).map_err(hello_err)?;
        Ok(out.to_vec())
    }

    pub fn delete(name: &str) {
        let name = HSTRING::from(name);
        if let Err(e) = KeyCredentialManager::DeleteAsync(&name).and_then(|op| op.join()) {
            tracing::warn!(error = %e.message(), "cannot delete Windows Hello key");
        }
    }

    /// The Hello dialog of a desktop app often opens behind its window.
    /// Bring it to the front once it appears (best effort, like KeePassXC).
    fn raise_dialog() {
        std::thread::spawn(|| {
            let until = Instant::now() + Duration::from_secs(3);
            while Instant::now() < until {
                // SAFETY: plain Win32 calls with static, NUL-terminated class names.
                unsafe {
                    if let Ok(hwnd) = FindWindowW(w!("Credential Dialog Xaml Host"), None)
                        && !hwnd.is_invalid()
                    {
                        let _ = SetForegroundWindow(hwnd);
                        return;
                    }
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::error::{DesktopError, Result};

    pub fn available() -> bool {
        false
    }

    pub fn sign(_name: &str, _challenge: &[u8], _create: bool) -> Result<Vec<u8>> {
        Err(DesktopError::invalid(
            "Windows Hello is only available on Windows",
        ))
    }

    pub fn delete(_name: &str) {}
}
