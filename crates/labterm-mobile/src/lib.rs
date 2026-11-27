//! Labterm mobile façade: the API the Android app talks to, exported through
//! UniFFI. The Rust side owns storage, encryption, SSH and terminal
//! emulation; Kotlin owns the Keystore-wrapped master key, the UI and the
//! foreground service.
//!
//! Secrets policy: list/card records never carry passwords, passphrases or
//! private keys. The only calls that return private material are
//! [`LabtermApp::export_private_key`] (explicit user action) and prompt
//! answers travelling *into* Rust.

#![forbid(unsafe_code)]

uniffi::setup_scaffolding!("termoso");

mod account;
mod ai;
mod app;
mod autocomplete;
mod backup;
mod connect;
mod dto;
mod error;
mod fido2;
mod forward;
mod import;
mod keyexport;
mod keys;
mod linux;
mod live;
mod netbird;
mod oidc;
mod policy_view;
mod presence;
mod quiet;
mod s3;
mod serial;
mod session;
mod settings;
mod sftp;
mod snippets;
mod sshid;
mod team;
mod terminal;
mod themes;
mod webdav;

pub use account::*;
pub use ai::{AiStatusCard, AiSuggestionCard, AiTarget};
pub use app::*;
pub use autocomplete::{SuggestionItem, SuggestionKind};
pub use backup::{
    BackupCard, BackupFile, BackupKindCount, BackupRestoreResult, BackupVaultCard,
    backup_min_password,
};
pub use connect::{
    ConnectStage, HostKeyChoice, InteractiveQuestionInfo, PromptAnswer, PromptRequest,
};
pub use dto::*;
pub use error::{MobileError, Result};
pub use fido2::{
    Fido2DeviceCard, Fido2Devices, Fido2GenerateDraft, Fido2Listener, Fido2LoadDraft, Fido2NfcLink,
    Fido2Transport, Fido2UsbLink, SecurityKeyCard, SkKeyAlgorithm,
};
pub use forward::{
    PfKind, PfRuleDraft, PfRuleItem, PfTunnel, TunnelListener, TunnelState, TunnelStats,
};
pub use keys::{KeyMods, SpecialKey, control_code, encode_key, encode_text};
pub use live::{LiveEndReason, LiveListener, LiveParticipantCard, LiveShare, is_live_link};
pub use netbird::{
    TransportDetails, TransportDnsGroup, TransportHost, TransportHostTest, TransportItem,
    TransportPeer, TransportState,
};
pub use oidc::{OidcKeyChoice, OidcProviderItem, OidcSessionItem, UrlOpener, set_url_opener};
pub use presence::{PresenceEntryCard, PresenceSessionCard, TeamPresenceCard};
pub use session::*;
pub use settings::{KeyGroup, MobileSettings, PanelKeyDef};
pub use sftp::*;
pub use snippets::{SnippetDraft, SnippetItem, SnippetPackageItem, SnippetRun};
pub use sshid::{
    DeviceKeyCard, SshIdKeyCard, SshIdKeyKind, SshIdView, sshid_handle_valid,
    sshid_provision_command, sshid_type_is_hardware, sshid_type_label,
};
pub use team::*;
pub use terminal::{
    CELL_BYTES, CursorStyle, Emulator, GridFrame, GridSnapshot, TerminalPalette, flag,
};
pub use themes::{LABTERM_DARK, LABTERM_LIGHT, TerminalTheme, terminal_theme, terminal_themes};
