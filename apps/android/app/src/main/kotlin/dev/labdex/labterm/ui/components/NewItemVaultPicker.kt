package dev.labdex.labterm.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.ui.res.stringResource
import dev.labdex.labterm.R
import dev.labdex.labterm.core.VaultAccess
import dev.labdex.labterm.core.VaultInfo
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.ui.vault.vaultLabel

/** Vaults a new item can go into: unlocked and writable (plus the current pick). */
fun writableVaults(vaults: List<VaultInfo>, selected: String?): List<VaultInfo> =
    vaults.filter { (!it.locked && it.access != VaultAccess.VIEW) || it.id == selected }

/** Loads [writableVaults] for pages that don't hold the vault list themselves. */
@Composable
fun rememberWritableVaults(repo: VaultRepository): List<VaultInfo> {
    val vaults by produceState(emptyList<VaultInfo>(), repo) {
        value = runCatching { repo.read { vaults() } }.getOrDefault(emptyList())
    }
    return writableVaults(vaults, null)
}

/**
 * "Vault" picker on new-item pages: the active vault is preselected, any vault
 * you can write to can be chosen. Hidden when there is only one.
 */
@Composable
fun NewItemVaultPicker(vaults: List<VaultInfo>, selected: String, onPick: (String) -> Unit) {
    val options = writableVaults(vaults, selected)
    if (options.size < 2) return
    PickerRow(
        label = stringResource(R.string.vault),
        value = options.firstOrNull { it.id == selected }?.let { vaultLabel(it) } ?: "",
        options = options.map { it.id to vaultLabel(it) },
        selected = selected,
        onPick = { id -> id?.let(onPick) },
        empty = null,
    )
}
