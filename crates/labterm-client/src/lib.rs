//! Client façades shared by the Labterm shells (Tauri desktop, Android).
//!
//! Everything here is pure `Store` logic: the UI edits flat forms
//! ([`hosts::HostForm`], [`keychain::GenerateForm`], …) and Rust maps them to
//! the sync entities, resolves group inheritance and keeps secrets from
//! leaking into list DTOs. Nothing in this crate touches the network or a
//! window system, except the opt-in [`oidc`] pre-connect hook (feature
//! `oidc-cert`), which runs an OpenID login in the browser.

#![forbid(unsafe_code)]

pub mod access;
pub mod backup;
pub mod error;
pub mod hosts;
pub mod keychain;
#[cfg(feature = "oidc-cert")]
pub mod oidc;
pub mod overlays;
pub mod policy_view;
pub mod snippets;
pub mod transports;
pub mod trust;
pub mod vaultmove;
#[cfg(feature = "vpn")]
pub mod vpn;

pub use error::{ClientError, Result};
