//! FIDO2 security keys through the Windows WebAuthn API (`webauthn.dll`).
//!
//! Since Windows 10 1903 only elevated processes may open FIDO HID devices;
//! everyone else has to go through `webauthn.dll`, which shows the system
//! dialog (pick a key, enter the PIN, touch it). This crate is the thin,
//! `unsafe` FFI layer: plain bytes in, plain bytes out. Turning them into
//! OpenSSH `sk-*` keys and signatures happens in `labterm-core::fido2`.
//!
//! The DLL is loaded at run time, so the app still starts on a Windows
//! without it; [`api_version`] is then `None`. On other platforms every
//! call reports [`Error::Unavailable`], which keeps callers free of `cfg`.

#![deny(unsafe_op_in_unsafe_fn)]

/// What can go wrong talking to `webauthn.dll`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// Not Windows, or a Windows without the WebAuthn API.
    #[error("Windows WebAuthn API not available")]
    Unavailable,
    /// The user closed the Windows Security dialog, or it timed out.
    #[error("cancelled in the Windows Security dialog")]
    Cancelled,
    /// Anything else, with the name Windows gives the error.
    #[error("Windows WebAuthn: {0}")]
    Other(String),
}

/// Whether the authenticator must verify the user (PIN / biometric).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserVerification {
    /// Always (OpenSSH `verify-required`).
    Required,
    /// Only when the token insists.
    Discouraged,
}

/// `authenticatorGetAssertion` input.
#[derive(Debug, Clone)]
pub struct GetAssertion<'a> {
    /// Relying party id (OpenSSH: the key's application, `ssh:…`).
    pub rp_id: &'a str,
    /// Bytes Windows hashes with SHA-256 into `clientDataHash`.
    pub client_data: &'a [u8],
    /// The credential to use (OpenSSH key handle).
    pub credential_id: &'a [u8],
    /// PIN / biometric requirement.
    pub user_verification: UserVerification,
}

/// Raw assertion: authenticator data and signature over
/// `authData ‖ clientDataHash`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assertion {
    /// Authenticator data as signed.
    pub auth_data: Vec<u8>,
    /// Signature (Ed25519 raw, ES256 DER).
    pub signature: Vec<u8>,
}

/// `authenticatorMakeCredential` input.
#[derive(Debug, Clone)]
pub struct MakeCredential<'a> {
    /// Relying party id (`ssh:…`).
    pub rp_id: &'a str,
    /// User handle.
    pub user_id: &'a [u8],
    /// User name (also used as display name).
    pub user_name: &'a str,
    /// Bytes Windows hashes into `clientDataHash` (a random challenge).
    pub client_data: &'a [u8],
    /// COSE algorithm: -7 ES256, -8 EdDSA.
    pub cose_alg: i32,
    /// Store the credential on the token (discoverable).
    pub resident: bool,
    /// PIN / biometric requirement.
    pub user_verification: UserVerification,
    /// Enable the `hmac-secret` extension on the new credential.
    pub hmac_secret: bool,
}

/// A new credential: its id and the authenticator data with the attested
/// credential (COSE public key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MadeCredential {
    /// Credential id (OpenSSH key handle).
    pub credential_id: Vec<u8>,
    /// Authenticator data including `attestedCredentialData`.
    pub auth_data: Vec<u8>,
}

/// WebAuthn API version, `None` when the API is not there.
pub fn api_version() -> Option<u32> {
    imp::api_version()
}

/// Sign with an existing credential. Blocks while the Windows dialog is up.
pub fn get_assertion(req: &GetAssertion<'_>) -> Result<Assertion, Error> {
    imp::get_assertion(req)
}

/// `hmac-secret` from whichever of `credentials` (id, raw CTAP salt) the
/// chosen key holds, without user verification: that credential's id and
/// the 32-byte output. Needs API version 4 (Windows 11). Blocks while the
/// dialog is up.
pub fn hmac_secret(
    rp_id: &str,
    credentials: &[(&[u8], &[u8; 32])],
) -> Result<(Vec<u8>, Vec<u8>), Error> {
    imp::hmac_secret(rp_id, credentials)
}

/// Create a credential on a security key. Blocks while the dialog is up.
pub fn make_credential(req: &MakeCredential<'_>) -> Result<MadeCredential, Error> {
    imp::make_credential(req)
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn api_version() -> Option<u32> {
        None
    }

    pub fn get_assertion(_: &GetAssertion<'_>) -> Result<Assertion, Error> {
        Err(Error::Unavailable)
    }

    pub fn make_credential(_: &MakeCredential<'_>) -> Result<MadeCredential, Error> {
        Err(Error::Unavailable)
    }

    pub fn hmac_secret(_: &str, _: &[(&[u8], &[u8; 32])]) -> Result<(Vec<u8>, Vec<u8>), Error> {
        Err(Error::Unavailable)
    }
}

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;

    use windows::Win32::Foundation::{HMODULE, HWND};
    use windows::Win32::Networking::WindowsWebServices::*;
    use windows::Win32::System::LibraryLoader::{
        GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    use windows::core::{BOOL, HRESULT, PCWSTR, s, w};

    use super::*;

    /// `NTE_USER_CANCELLED`: dialog closed by the user.
    const NTE_USER_CANCELLED: HRESULT = HRESULT(0x8009_0036_u32 as i32);
    /// First API version whose get-assertion options carry hmac-secret salts.
    const HMAC_SALT_API: u32 = WEBAUTHN_API_VERSION_4;
    /// Two minutes, like a browser.
    const TIMEOUT_MS: u32 = 120_000;

    type GetApiVersionFn = unsafe extern "system" fn() -> u32;
    type GetAssertionFn = unsafe extern "system" fn(
        HWND,
        PCWSTR,
        *const WEBAUTHN_CLIENT_DATA,
        *const WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS,
        *mut *mut WEBAUTHN_ASSERTION,
    ) -> HRESULT;
    type FreeAssertionFn = unsafe extern "system" fn(*const WEBAUTHN_ASSERTION);
    type MakeCredentialFn = unsafe extern "system" fn(
        HWND,
        *const WEBAUTHN_RP_ENTITY_INFORMATION,
        *const WEBAUTHN_USER_ENTITY_INFORMATION,
        *const WEBAUTHN_COSE_CREDENTIAL_PARAMETERS,
        *const WEBAUTHN_CLIENT_DATA,
        *const WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS,
        *mut *mut WEBAUTHN_CREDENTIAL_ATTESTATION,
    ) -> HRESULT;
    type FreeAttestationFn = unsafe extern "system" fn(*const WEBAUTHN_CREDENTIAL_ATTESTATION);
    type GetErrorNameFn = unsafe extern "system" fn(HRESULT) -> PCWSTR;

    struct Api {
        version: u32,
        get_assertion: GetAssertionFn,
        free_assertion: FreeAssertionFn,
        make_credential: MakeCredentialFn,
        free_attestation: FreeAttestationFn,
        error_name: GetErrorNameFn,
    }

    fn api() -> Option<&'static Api> {
        static API: OnceLock<Option<Api>> = OnceLock::new();
        API.get_or_init(load).as_ref()
    }

    fn load() -> Option<Api> {
        // SAFETY: loading a system DLL from System32 only (no search-path
        // planting) and casting each export to its documented signature
        // (webauthn.h). The module is never freed, so the pointers stay valid.
        unsafe {
            let dll: HMODULE =
                LoadLibraryExW(w!("webauthn.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
            macro_rules! sym {
                ($name:literal, $ty:ty) => {
                    std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(
                        GetProcAddress(dll, s!($name))?,
                    )
                };
            }
            let version: GetApiVersionFn = sym!("WebAuthNGetApiVersionNumber", GetApiVersionFn);
            Some(Api {
                version: version(),
                get_assertion: sym!("WebAuthNAuthenticatorGetAssertion", GetAssertionFn),
                free_assertion: sym!("WebAuthNFreeAssertion", FreeAssertionFn),
                make_credential: sym!("WebAuthNAuthenticatorMakeCredential", MakeCredentialFn),
                free_attestation: sym!("WebAuthNFreeCredentialAttestation", FreeAttestationFn),
                error_name: sym!("WebAuthNGetErrorName", GetErrorNameFn),
            })
        }
    }

    pub fn api_version() -> Option<u32> {
        api().map(|a| a.version)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn uv(v: UserVerification) -> u32 {
        match v {
            UserVerification::Required => WEBAUTHN_USER_VERIFICATION_REQUIREMENT_REQUIRED,
            UserVerification::Discouraged => WEBAUTHN_USER_VERIFICATION_REQUIREMENT_DISCOURAGED,
        }
    }

    fn error(api: &Api, hr: HRESULT) -> Error {
        if hr == NTE_USER_CANCELLED {
            return Error::Cancelled;
        }
        // SAFETY: returns a static, NUL-terminated string (or null).
        let name = unsafe {
            let p = (api.error_name)(hr);
            if p.is_null() {
                None
            } else {
                p.to_string().ok()
            }
        };
        match name.as_deref() {
            // What Windows reports for a closed or timed-out dialog.
            Some("NotAllowedError") => Error::Cancelled,
            Some(n) => Error::Other(format!("{n} (0x{:08X})", hr.0 as u32)),
            None => Error::Other(format!("0x{:08X}", hr.0 as u32)),
        }
    }

    /// Copy `len` bytes from a pointer Windows owns.
    ///
    /// # Safety
    /// `ptr` must be valid for `len` bytes (or `len` must be 0).
    unsafe fn bytes(ptr: *const u8, len: u32) -> Vec<u8> {
        if ptr.is_null() || len == 0 {
            return Vec::new();
        }
        // SAFETY: guaranteed by the caller.
        unsafe { std::slice::from_raw_parts(ptr, len as usize) }.to_vec()
    }

    fn client_data(data: &[u8]) -> WEBAUTHN_CLIENT_DATA {
        WEBAUTHN_CLIENT_DATA {
            dwVersion: WEBAUTHN_CLIENT_DATA_CURRENT_VERSION,
            cbClientDataJSON: data.len() as u32,
            // Windows only reads it.
            pbClientDataJSON: data.as_ptr() as *mut u8,
            pwszHashAlgId: WEBAUTHN_HASH_ALGORITHM_SHA_256,
        }
    }

    pub fn get_assertion(req: &GetAssertion<'_>) -> Result<Assertion, Error> {
        let api = api().ok_or(Error::Unavailable)?;
        let rp = wide(req.rp_id);
        let cd = client_data(req.client_data);
        let mut cred = WEBAUTHN_CREDENTIAL {
            dwVersion: WEBAUTHN_CREDENTIAL_CURRENT_VERSION,
            cbId: req.credential_id.len() as u32,
            pbId: req.credential_id.as_ptr() as *mut u8,
            pwszCredentialType: WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY,
        };
        // Version 1 of the options: everything we set is in it, and every
        // webauthn.dll since 1903 accepts it.
        let opts = WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS {
            dwVersion: 1,
            dwTimeoutMilliseconds: TIMEOUT_MS,
            CredentialList: WEBAUTHN_CREDENTIALS {
                cCredentials: 1,
                pCredentials: &mut cred,
            },
            dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_CROSS_PLATFORM,
            dwUserVerificationRequirement: uv(req.user_verification),
            ..Default::default()
        };
        let mut out: *mut WEBAUTHN_ASSERTION = std::ptr::null_mut();
        // SAFETY: every pointer refers to a local that outlives the call;
        // `out` is freed with the matching free function below.
        unsafe {
            let hwnd = GetForegroundWindow();
            let hr = (api.get_assertion)(hwnd, PCWSTR(rp.as_ptr()), &cd, &opts, &mut out);
            if hr.is_err() || out.is_null() {
                return Err(error(api, hr));
            }
            let a = &*out;
            let result = Assertion {
                auth_data: bytes(a.pbAuthenticatorData, a.cbAuthenticatorData),
                signature: bytes(a.pbSignature, a.cbSignature),
            };
            (api.free_assertion)(out);
            Ok(result)
        }
    }

    pub fn hmac_secret(
        rp_id: &str,
        credentials: &[(&[u8], &[u8; 32])],
    ) -> Result<(Vec<u8>, Vec<u8>), Error> {
        let api = api().ok_or(Error::Unavailable)?;
        if api.version < HMAC_SALT_API {
            return Err(Error::Other(
                "this Windows is too old for security key unlock (needs Windows 11)".into(),
            ));
        }
        let rp = wide(rp_id);
        // Windows hashes the client data into a clientDataHash we never use.
        let challenge = [0u8; 32];
        let cd = client_data(&challenge);
        // Owned copies: Windows takes mutable pointers, and every buffer must
        // outlive the call.
        let mut ids: Vec<Vec<u8>> = credentials.iter().map(|(id, _)| id.to_vec()).collect();
        let mut salts: Vec<[u8; 32]> = credentials.iter().map(|(_, s)| **s).collect();
        let mut creds: Vec<WEBAUTHN_CREDENTIAL> = ids
            .iter_mut()
            .map(|id| WEBAUTHN_CREDENTIAL {
                dwVersion: WEBAUTHN_CREDENTIAL_CURRENT_VERSION,
                cbId: id.len() as u32,
                pbId: id.as_mut_ptr(),
                pwszCredentialType: WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY,
            })
            .collect();
        let mut salt_structs: Vec<WEBAUTHN_HMAC_SECRET_SALT> = salts
            .iter_mut()
            .map(|s| WEBAUTHN_HMAC_SECRET_SALT {
                cbFirst: s.len() as u32,
                pbFirst: s.as_mut_ptr(),
                ..Default::default()
            })
            .collect();
        let mut per_cred: Vec<WEBAUTHN_CRED_WITH_HMAC_SECRET_SALT> = ids
            .iter_mut()
            .zip(salt_structs.iter_mut())
            .map(|(id, salt)| WEBAUTHN_CRED_WITH_HMAC_SECRET_SALT {
                cbCredID: id.len() as u32,
                pbCredID: id.as_mut_ptr(),
                pHmacSecretSalt: salt,
            })
            .collect();
        let mut values = WEBAUTHN_HMAC_SECRET_SALT_VALUES {
            pGlobalHmacSalt: std::ptr::null_mut(),
            cCredWithHmacSecretSaltList: per_cred.len() as u32,
            pCredWithHmacSecretSaltList: per_cred.as_mut_ptr(),
        };
        let opts = WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS {
            dwVersion: WEBAUTHN_AUTHENTICATOR_GET_ASSERTION_OPTIONS_VERSION_6,
            dwTimeoutMilliseconds: TIMEOUT_MS,
            CredentialList: WEBAUTHN_CREDENTIALS {
                cCredentials: creds.len() as u32,
                pCredentials: creds.as_mut_ptr(),
            },
            dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_CROSS_PLATFORM,
            dwUserVerificationRequirement: WEBAUTHN_USER_VERIFICATION_REQUIREMENT_DISCOURAGED,
            // Raw CTAP salts: Windows must not hash them as WebAuthn PRF inputs.
            dwFlags: WEBAUTHN_AUTHENTICATOR_HMAC_SECRET_VALUES_FLAG,
            pHmacSecretSaltValues: &mut values,
            ..Default::default()
        };
        let mut out: *mut WEBAUTHN_ASSERTION = std::ptr::null_mut();
        // SAFETY: every pointer refers to a local that outlives the call;
        // `out` is freed with the matching free function below.
        unsafe {
            let hwnd = GetForegroundWindow();
            let hr = (api.get_assertion)(hwnd, PCWSTR(rp.as_ptr()), &cd, &opts, &mut out);
            if hr.is_err() || out.is_null() {
                return Err(error(api, hr));
            }
            let a = &*out;
            let secret = if a.dwVersion >= WEBAUTHN_ASSERTION_VERSION_3 && !a.pHmacSecret.is_null()
            {
                let h = &*a.pHmacSecret;
                bytes(h.pbFirst, h.cbFirst)
            } else {
                Vec::new()
            };
            let id = bytes(a.Credential.pbId, a.Credential.cbId);
            (api.free_assertion)(out);
            if secret.len() != 32 {
                return Err(Error::Other("the key returned no hmac-secret".into()));
            }
            let id = if id.is_empty() {
                match credentials {
                    [(only, _)] => only.to_vec(),
                    _ => {
                        return Err(Error::Other(
                            "Windows did not say which key answered".into(),
                        ));
                    }
                }
            } else {
                id
            };
            Ok((id, secret))
        }
    }

    pub fn make_credential(req: &MakeCredential<'_>) -> Result<MadeCredential, Error> {
        let api = api().ok_or(Error::Unavailable)?;
        let rp_id = wide(req.rp_id);
        let user_name = wide(req.user_name);
        let rp = WEBAUTHN_RP_ENTITY_INFORMATION {
            dwVersion: WEBAUTHN_RP_ENTITY_INFORMATION_CURRENT_VERSION,
            pwszId: PCWSTR(rp_id.as_ptr()),
            pwszName: PCWSTR(rp_id.as_ptr()),
            pwszIcon: PCWSTR::null(),
        };
        let user = WEBAUTHN_USER_ENTITY_INFORMATION {
            dwVersion: WEBAUTHN_USER_ENTITY_INFORMATION_CURRENT_VERSION,
            cbId: req.user_id.len() as u32,
            pbId: req.user_id.as_ptr() as *mut u8,
            pwszName: PCWSTR(user_name.as_ptr()),
            pwszIcon: PCWSTR::null(),
            pwszDisplayName: PCWSTR(user_name.as_ptr()),
        };
        let mut param = WEBAUTHN_COSE_CREDENTIAL_PARAMETER {
            dwVersion: WEBAUTHN_COSE_CREDENTIAL_PARAMETER_CURRENT_VERSION,
            pwszCredentialType: WEBAUTHN_CREDENTIAL_TYPE_PUBLIC_KEY,
            lAlg: req.cose_alg,
        };
        let params = WEBAUTHN_COSE_CREDENTIAL_PARAMETERS {
            cCredentialParameters: 1,
            pCredentialParameters: &mut param,
        };
        let cd = client_data(req.client_data);
        let mut enable = BOOL::from(true);
        let mut ext = WEBAUTHN_EXTENSION {
            pwszExtensionIdentifier: WEBAUTHN_EXTENSIONS_IDENTIFIER_HMAC_SECRET,
            cbExtension: std::mem::size_of::<BOOL>() as u32,
            pvExtension: (&mut enable as *mut BOOL).cast(),
        };
        let extensions = if req.hmac_secret {
            WEBAUTHN_EXTENSIONS {
                cExtensions: 1,
                pExtensions: &mut ext,
            }
        } else {
            WEBAUTHN_EXTENSIONS::default()
        };
        let opts = WEBAUTHN_AUTHENTICATOR_MAKE_CREDENTIAL_OPTIONS {
            dwVersion: 1,
            dwTimeoutMilliseconds: TIMEOUT_MS,
            dwAuthenticatorAttachment: WEBAUTHN_AUTHENTICATOR_ATTACHMENT_CROSS_PLATFORM,
            bRequireResidentKey: BOOL::from(req.resident),
            dwUserVerificationRequirement: uv(req.user_verification),
            dwAttestationConveyancePreference: WEBAUTHN_ATTESTATION_CONVEYANCE_PREFERENCE_NONE,
            Extensions: extensions,
            ..Default::default()
        };
        let mut out: *mut WEBAUTHN_CREDENTIAL_ATTESTATION = std::ptr::null_mut();
        // SAFETY: as in `get_assertion`.
        unsafe {
            let hwnd = GetForegroundWindow();
            let hr = (api.make_credential)(hwnd, &rp, &user, &params, &cd, &opts, &mut out);
            if hr.is_err() || out.is_null() {
                return Err(error(api, hr));
            }
            let a = &*out;
            let result = MadeCredential {
                credential_id: bytes(a.pbCredentialId, a.cbCredentialId),
                auth_data: bytes(a.pbAuthenticatorData, a.cbAuthenticatorData),
            };
            (api.free_attestation)(out);
            Ok(result)
        }
    }
}
