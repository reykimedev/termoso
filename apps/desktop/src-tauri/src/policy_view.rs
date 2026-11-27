//! Policy viewer: the effective setup of every host in a vault, computed on
//! this device (see `labterm_client::policy_view`).

use labterm_client::policy_view::{self, VaultPolicy};
use tauri::State;
use uuid::Uuid;

use crate::error::Result;
use crate::state::AppState;

#[tauri::command]
pub async fn vault_policy(state: State<'_, AppState>, vault_id: Uuid) -> Result<VaultPolicy> {
    policy_view::vault_policy(&*state.store()?, vault_id)
}
