//! Wire types shared by the Labterm server and all clients.
//!
//! Every type here is plain `serde` data. Fields that hold user secrets are
//! *always* ciphertext envelopes produced by `labterm-crypto` (base64 strings);
//! the server treats them as opaque.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod account;
pub mod admin;
pub mod ai;
pub mod auth;
pub mod automation;
pub mod bridge;
pub mod ca;
pub mod collection;
pub mod entities;
pub mod error;
pub mod live;
pub mod logs;
pub mod sshid;
pub mod sync;
pub mod team;
pub mod timestamp;
pub mod vault;
pub mod ws;

/// API version prefix for all REST routes.
pub const API_PREFIX: &str = "/api/v1";

/// Header carrying the device session token.
pub const AUTH_HEADER: &str = "authorization";

/// Macro to derive the OpenAPI schema only when the `openapi` feature is on.
macro_rules! schema {
    ($(#[$m:meta])* $vis:vis struct $name:ident $($rest:tt)*) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        $(#[$m])*
        $vis struct $name $($rest)*
    };
    ($(#[$m:meta])* $vis:vis enum $name:ident $($rest:tt)*) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        $(#[$m])*
        $vis enum $name $($rest)*
    };
}
pub(crate) use schema;

/// Reads a `LABTERM_*` setting. Deployments made before the rename still set
/// the old `TERMOSO_*` names, so those are read when the new one is unset. rename:keep
pub fn setting(name: &str) -> Result<String, std::env::VarError> {
    setting_from(name, |n| std::env::var(n))
}

fn setting_from(
    name: &str,
    get: impl Fn(&str) -> Result<String, std::env::VarError>,
) -> Result<String, std::env::VarError> {
    get(name).or_else(|e| match name.strip_prefix("LABTERM_") {
        Some(rest) => get(&format!("TERMOSO_{rest}")), // rename:keep
        None => Err(e),
    })
}

#[cfg(test)]
mod setting_tests {
    use std::env::VarError;

    #[test]
    fn falls_back_to_the_old_name() {
        let env = |n: &str| match n {
            "TERMOSO_OLD" | "TERMOSO_BOTH" => Ok("old".to_string()), // rename:keep
            "LABTERM_BOTH" => Ok("new".to_string()),
            _ => Err(VarError::NotPresent),
        };
        assert_eq!(super::setting_from("LABTERM_OLD", env).unwrap(), "old");
        assert_eq!(super::setting_from("LABTERM_BOTH", env).unwrap(), "new");
        assert!(super::setting_from("LABTERM_NONE", env).is_err());
        assert!(super::setting_from("OTHER", env).is_err());
    }
}
