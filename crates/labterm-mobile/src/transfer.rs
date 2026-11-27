//! Move or copy hosts and whole groups into another vault, with a preview
//! (`labterm_client::vaultmove`).

use labterm_client::vaultmove::{self, TransferMode, TransferPlan, TransferRequest};

use crate::app::LabtermApp;
use crate::dto::{parse_id, parse_ids, parse_opt_id};
use crate::error::Result;

/// What a transfer does to something the hosts use.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TransferDependencyCard {
    /// `identity`, `ssh_key`, `snippet`, `proxy`, `host_chain`, `port_knocking`,
    /// `transport`, `color_scheme`, `tag`, `pf_rule` or `host_overlay`.
    pub kind: String,
    pub id: String,
    pub label: String,
    /// `reuse`, `copy`, `leave_out`, `unlink`, `move_along`, `rewrite`,
    /// `remove` or `breaks`.
    pub action: String,
}

/// Why a transfer is refused.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TransferBlockedCard {
    pub id: Option<String>,
    pub label: String,
    /// `nothing`, `same_vault`, `mixed_vaults`, `target_locked`,
    /// `target_read_only`, `target_group`, `source_read_only`,
    /// `connect_only`, `hidden` or `restricted`.
    pub reason: String,
}

/// The preview of a transfer.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TransferPlanCard {
    pub source_vault_id: Option<String>,
    pub target_vault_id: String,
    pub hosts: u32,
    pub groups: u32,
    pub blocked: Vec<TransferBlockedCard>,
    pub dependencies: Vec<TransferDependencyCard>,
}

fn snake<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn card(p: TransferPlan) -> TransferPlanCard {
    TransferPlanCard {
        source_vault_id: p.source_vault_id.map(|v| v.to_string()),
        target_vault_id: p.target_vault_id.to_string(),
        hosts: p.hosts,
        groups: p.groups,
        blocked: p
            .blocked
            .into_iter()
            .map(|b| TransferBlockedCard {
                id: b.id.map(|i| i.to_string()),
                label: b.label,
                reason: snake(&b.reason),
            })
            .collect(),
        dependencies: p
            .dependencies
            .into_iter()
            .map(|d| TransferDependencyCard {
                kind: d.kind,
                id: d.id.to_string(),
                label: d.label,
                action: snake(&d.action),
            })
            .collect(),
    }
}

fn request(
    host_ids: &[String],
    group_ids: &[String],
    vault_id: &str,
    group_id: &Option<String>,
    mv: bool,
    with_credentials: bool,
) -> Result<TransferRequest> {
    Ok(TransferRequest {
        host_ids: parse_ids(host_ids)?,
        group_ids: parse_ids(group_ids)?,
        vault_id: parse_id(vault_id)?,
        group_id: parse_opt_id(group_id)?,
        mode: if mv {
            TransferMode::Move
        } else {
            TransferMode::Copy
        },
        with_credentials,
    })
}

#[uniffi::export]
impl LabtermApp {
    /// Preview moving (`mv`) or copying hosts and groups into `vault_id`
    /// (into `group_id` there, or the top level).
    pub fn vault_transfer_plan(
        &self,
        host_ids: Vec<String>,
        group_ids: Vec<String>,
        vault_id: String,
        group_id: Option<String>,
        mv: bool,
        with_credentials: bool,
    ) -> Result<TransferPlanCard> {
        let req = request(
            &host_ids,
            &group_ids,
            &vault_id,
            &group_id,
            mv,
            with_credentials,
        )?;
        Ok(card(vaultmove::plan(&self.store, &req)?))
    }

    /// Move or copy hosts and groups into `vault_id`; refused as a whole when
    /// the preview has blockers.
    pub fn vault_transfer(
        &self,
        host_ids: Vec<String>,
        group_ids: Vec<String>,
        vault_id: String,
        group_id: Option<String>,
        mv: bool,
        with_credentials: bool,
    ) -> Result<TransferPlanCard> {
        let req = request(
            &host_ids,
            &group_ids,
            &vault_id,
            &group_id,
            mv,
            with_credentials,
        )?;
        Ok(card(vaultmove::apply(&self.store, &req)?.plan))
    }
}
