package dev.labdex.labterm.ui.vault

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R
import dev.labdex.labterm.core.VaultInfo

/**
 * Top-bar title that names the current vault and, when there is more than
 * one, opens a bottom sheet to switch — like the vault selector in Termius.
 * Pinned vaults (synced per user) come first; each row has a pin toggle.
 */
@Composable
fun VaultPickerTitle(
    vaults: List<VaultInfo>,
    selectedId: String?,
    onPick: (String) -> Unit,
    pinned: List<String> = emptyList(),
    onTogglePin: ((String) -> Unit)? = null,
) {
    val current = vaults.firstOrNull { it.id == selectedId }
    val switchable = vaults.size > 1
    var open by remember { mutableStateOf(false) }
    Row(
        Modifier
            .clip(RoundedCornerShape(8.dp))
            .clickable(enabled = switchable) { open = true }
            .padding(horizontal = 8.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            current?.let { vaultLabel(it) } ?: stringResource(R.string.vault),
            style = MaterialTheme.typography.titleLarge,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        if (switchable) {
            Icon(
                Icons.Filled.ExpandMore,
                contentDescription = stringResource(R.string.vaults),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
    if (open) {
        VaultPickerSheet(
            vaults = vaults,
            selectedId = selectedId,
            pinned = pinned,
            onPick = onPick,
            onTogglePin = onTogglePin,
            onDismiss = { open = false },
        )
    }
}

/** Pinned vaults first, in pin order, then the rest in their usual order. */
fun splitPinned(vaults: List<VaultInfo>, pinned: List<String>): Pair<List<VaultInfo>, List<VaultInfo>> {
    val byId = vaults.associateBy { it.id }
    val top = pinned.mapNotNull { byId[it] }
    val ids = top.map { it.id }.toSet()
    return top to vaults.filter { it.id !in ids }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun VaultPickerSheet(
    vaults: List<VaultInfo>,
    selectedId: String?,
    pinned: List<String>,
    onPick: (String) -> Unit,
    onTogglePin: ((String) -> Unit)?,
    onDismiss: () -> Unit,
) {
    val (top, rest) = splitPinned(vaults, pinned)
    ModalBottomSheet(onDismissRequest = onDismiss) {
        Text(
            stringResource(R.string.vaults),
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
        )
        LazyColumn(Modifier.navigationBarsPadding().padding(bottom = 16.dp)) {
            if (top.isNotEmpty()) {
                item { SectionLabel(stringResource(R.string.pinned)) }
                items(top, key = { "p" + it.id }) { v ->
                    VaultRow(v, v.id == selectedId, true, onTogglePin, { onPick(v.id); onDismiss() })
                }
                if (rest.isNotEmpty()) item { SectionLabel(stringResource(R.string.all_vaults)) }
            }
            items(rest, key = { it.id }) { v ->
                VaultRow(v, v.id == selectedId, false, onTogglePin, { onPick(v.id); onDismiss() })
            }
        }
    }
}

@Composable
private fun SectionLabel(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 24.dp, end = 24.dp, top = 12.dp, bottom = 4.dp),
    )
}

@Composable
private fun VaultRow(
    v: VaultInfo,
    active: Boolean,
    isPinned: Boolean,
    onTogglePin: ((String) -> Unit)?,
    onClick: () -> Unit,
) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .selectable(selected = active, role = Role.RadioButton, onClick = onClick)
            .padding(start = 24.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                vaultLabel(v),
                style = MaterialTheme.typography.bodyLarge,
                color = if (active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (v.locked) {
                Text(
                    stringResource(R.string.waiting_for_a_key),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (active) {
            Spacer(Modifier.width(8.dp))
            Icon(Icons.Filled.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        }
        if (onTogglePin != null) {
            IconButton(onClick = { onTogglePin(v.id) }) {
                Icon(
                    if (isPinned) Icons.Filled.PushPin else Icons.Outlined.PushPin,
                    contentDescription = stringResource(if (isPinned) R.string.unpin_vault else R.string.pin_vault),
                    tint = if (isPinned) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}
