package dev.labdex.labterm.ui.hosts

import androidx.annotation.StringRes
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dev.labdex.labterm.R
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.core.ImportPreviewItem
import dev.labdex.labterm.core.ImportedHostItem
import dev.labdex.labterm.core.GroupItem
import dev.labdex.labterm.core.HostItem
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

enum class HostSort(@StringRes val label: Int) {
    NAME(R.string.name),
    LAST_CONNECTED(R.string.last_connected),
    UPDATED(R.string.recently_updated),
}

data class HostsUiState(
    val loading: Boolean = true,
    val group: GroupItem? = null,
    val groups: List<GroupItem> = emptyList(),
    val hosts: List<HostItem> = emptyList(),
    val allGroups: List<GroupItem> = emptyList(),
    val query: String = "",
    val sort: HostSort = HostSort.NAME,
    val selected: Set<String> = emptySet(),
    val error: String? = null,
) {
    val selecting: Boolean get() = selected.isNotEmpty()

    val visibleGroups: List<GroupItem>
        get() = groups
            .filter { query.isBlank() || it.label.contains(query, ignoreCase = true) }
            .sortedBy { it.label.lowercase() }

    val visibleHosts: List<HostItem>
        get() {
            val q = query.trim()
            val filtered = if (q.isBlank()) {
                hosts
            } else {
                hosts.filter {
                    it.label.contains(q, true) || it.address.contains(q, true) ||
                        it.username.contains(q, true) || it.tags.any { t -> t.contains(q, true) }
                }
            }
            return when (sort) {
                HostSort.NAME -> filtered.sortedBy { it.label.ifBlank { it.address }.lowercase() }
                HostSort.LAST_CONNECTED -> filtered.sortedByDescending { it.lastConnected ?: Long.MIN_VALUE }
                HostSort.UPDATED -> filtered.sortedByDescending { it.updatedAt }
            }
        }
}

/**
 * One level of the host tree (a vault root or a group). Reloads whenever the
 * repository reports a mutation or the selected vault changes.
 */
class HostsViewModel(
    private val repo: VaultRepository,
    vaultId: StateFlow<String?>,
    private val groupId: String?,
    /** Show every host of the vault carrying this tag (label) instead of one group level. */
    private val tag: String? = null,
) : ViewModel() {
    private val _state = MutableStateFlow(HostsUiState())
    val state: StateFlow<HostsUiState> = _state.asStateFlow()
    private var currentVault: String? = null

    init {
        viewModelScope.launch {
            combine(repo.revision, vaultId) { _, v -> v }.collect { v ->
                currentVault = v
                reload(v)
            }
        }
    }

    /** Parse pasted text or a picked file with the shared parsers (ssh_config, PuTTY .reg, MobaXterm, SecureCRT, ZOC, CSV). */
    suspend fun parseImport(text: String, fileName: String): ImportPreviewItem =
        repo.read { importParse(text, fileName, null) }

    /** Import: creates one host per selected entry in the current vault/group. */
    fun importHosts(items: List<ImportedHostItem>) {
        val vault = currentVault ?: return
        viewModelScope.launch {
            runCatching {
                repo.write {
                    items.forEach { h ->
                        val d = newHostDraft(vault, groupId)
                        saveHost(
                            d.copy(
                                label = h.label.ifBlank { h.address },
                                address = h.address,
                                port = h.port,
                                username = h.username,
                                password = h.password,
                                notes = listOf(h.notes, h.groupPath.takeIf { it.isNotEmpty() }?.joinToString(" / ")?.let { "Imported from: $it" }.orEmpty())
                                    .filter { it.isNotBlank() }
                                    .joinToString("\n"),
                            ),
                        )
                    }
                }
            }
        }
    }

    private suspend fun reload(vaultId: String?) {
        if (vaultId == null) return
        runCatching {
            repo.read {
                val all = groups(vaultId)
                if (tag != null) {
                    Triple(all, emptyList(), hosts(vaultId).filter { h -> h.tags.any { it.equals(tag, ignoreCase = true) } })
                } else {
                    Triple(
                        all,
                        all.filter { it.parentId == groupId },
                        hosts(vaultId).filter { it.groupId == groupId },
                    )
                }
            }
        }.onSuccess { (all, groups, hosts) ->
            _state.update {
                it.copy(
                    loading = false,
                    group = all.firstOrNull { g -> g.id == groupId },
                    allGroups = all,
                    groups = groups,
                    hosts = hosts,
                    selected = it.selected.filterTo(HashSet()) { id -> hosts.any { h -> h.id == id } },
                    error = null,
                )
            }
        }.onFailure { e ->
            _state.update { it.copy(loading = false, error = e.userMessage()) }
        }
    }

    fun setQuery(q: String) = _state.update { it.copy(query = q) }

    fun setSort(sort: HostSort) = _state.update { it.copy(sort = sort) }

    fun toggle(id: String) = _state.update {
        it.copy(selected = if (id in it.selected) it.selected - id else it.selected + id)
    }

    fun selectAll() = _state.update { it.copy(selected = it.visibleHosts.map { h -> h.id }.toSet()) }

    fun clearSelection() = _state.update { it.copy(selected = emptySet()) }

    private fun mutate(block: suspend () -> Unit) {
        viewModelScope.launch {
            runCatching { block() }
                .onFailure { e -> _state.update { it.copy(error = e.userMessage()) } }
            clearSelection()
        }
    }

    fun delete(ids: List<String>) = mutate {
        repo.write { deleteHosts(ids) }
    }

    fun duplicate(ids: List<String>) = mutate {
        repo.write { ids.forEach { duplicateHost(it) } }
    }

    fun move(ids: List<String>, targetGroup: String?) = mutate {
        repo.write { moveHosts(ids, targetGroup) }
    }

    fun copy(ids: List<String>, vaultId: String, withCredentials: Boolean) = mutate {
        repo.write { copyHostsToVault(ids, vaultId, withCredentials) }
    }

    fun moveToVault(ids: List<String>, vaultId: String, withCredentials: Boolean) = mutate {
        repo.perms.require(currentVault, "entities.move")
        repo.write { moveHostsToVault(ids, vaultId, withCredentials) }
    }

    fun duplicateSelected() = duplicate(_state.value.selected.toList())

    fun createGroup(label: String) = mutate {
        val vault = currentVault ?: return@mutate
        repo.write { saveGroup(vault, null, label.trim(), groupId) }
    }

    fun renameGroup(group: GroupItem, label: String) = mutate {
        repo.write { saveGroup(group.vaultId, group.id, label.trim(), group.parentId) }
    }

    /** `withContents` deletes the hosts and sub-groups too; otherwise they move up one level. */
    fun deleteGroup(group: GroupItem, withContents: Boolean = false) = mutate {
        repo.write { if (withContents) deleteGroupRecursive(group.id) else deleteGroup(group.id) }
    }

    fun duplicateGroup(group: GroupItem) = mutate {
        repo.write { duplicateGroup(group.id) }
    }

    fun moveGroup(group: GroupItem, parentId: String?) = mutate {
        repo.write { saveGroup(group.vaultId, group.id, group.label, parentId) }
    }

    fun errorShown() = _state.update { it.copy(error = null) }
}
