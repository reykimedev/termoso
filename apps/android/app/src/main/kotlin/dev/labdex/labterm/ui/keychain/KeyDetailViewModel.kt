package dev.labdex.labterm.ui.keychain

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dev.labdex.labterm.R
import dev.labdex.labterm.data.VaultRepository
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str
import dev.labdex.labterm.core.KeyItem
import dev.labdex.labterm.core.OidcKeyChoice
import dev.labdex.labterm.core.OidcSessionItem
import dev.labdex.labterm.core.SecurityKeyCard
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class KeyDetailState(
    val loading: Boolean = true,
    val key: KeyItem? = null,
    val publicKey: String = "",
    val securityKey: SecurityKeyCard? = null,
    /** Provider choice of an opkssh key; null for other keys. */
    val oidc: OidcKeyChoice? = null,
    /** This device's sign-in of an opkssh key, while its certificate is usable. */
    val session: OidcSessionItem? = null,
    /** Label of the provider the key resolves to (named or the vault default). */
    val providerLabel: String? = null,
    val working: Boolean = false,
    val deleted: Boolean = false,
    val notice: String? = null,
)

/** One stored key: metadata, public half, and every mutation the Rust keychain offers. */
class KeyDetailViewModel(private val repo: VaultRepository, private val id: String) : ViewModel() {
    /** Team permission checks for this key's vault. */
    val perms get() = repo.perms
    private val _state = MutableStateFlow(KeyDetailState())
    val state: StateFlow<KeyDetailState> = _state.asStateFlow()
    private var deleting = false

    init {
        viewModelScope.launch { repo.revision.collect { load() } }
    }

    private suspend fun load() {
        if (deleting) return
        runCatching {
            repo.read {
                val key = keys(null).firstOrNull { it.id == id } ?: error(str(R.string.key_not_found))
                KeyDetailState(
                    loading = false,
                    key = key,
                    publicKey = runCatching { publicKey(id) }.getOrDefault(key.publicKey),
                    securityKey = if (key.keyType.startsWith("sk-")) runCatching { securityKeyInfo(id) }.getOrNull() else null,
                    oidc = runCatching { keyOidcChoice(id) }.getOrNull(),
                    session = runCatching { keyOidcSession(id) }.getOrNull(),
                ).let { st ->
                    val providers = if (st.oidc != null) runCatching { oidcProviders(key.vaultId) }.getOrDefault(emptyList()) else emptyList()
                    val chosen = st.oidc?.providerId?.let { pid -> providers.firstOrNull { it.id == pid } }
                        ?: providers.firstOrNull { it.isDefault } ?: providers.singleOrNull()
                    st.copy(providerLabel = chosen?.label)
                }
            }
        }.onSuccess { l -> _state.update { it.copy(loading = false, key = l.key, publicKey = l.publicKey, securityKey = l.securityKey, oidc = l.oidc, session = l.session, providerLabel = l.providerLabel) } }
            .onFailure { e -> _state.update { it.copy(loading = false, notice = e.userMessage()) } }
    }

    fun noticeShown() = _state.update { it.copy(notice = null) }

    private fun mutate(success: String?, block: suspend () -> Unit) {
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching { block() }
                .onSuccess { _state.update { it.copy(working = false, notice = success ?: it.notice) } }
                .onFailure { e -> _state.update { it.copy(working = false, notice = e.userMessage()) } }
        }
    }

    fun rename(label: String) = mutate(null) { repo.write { renameKey(id, label.trim()) } }

    fun changePassphrase(current: String?, next: String?, remember: Boolean) =
        mutate(if (next == null) str(R.string.passphrase_removed) else str(R.string.passphrase_changed)) {
            repo.write { changeKeyPassphrase(id, current, next, remember) }
        }

    fun setCertificate(text: String?) =
        mutate(if (text == null) str(R.string.certificate_removed) else str(R.string.certificate_attached)) {
            repo.write { setKeyCertificate(id, text?.trim()?.takeIf { it.isNotEmpty() }) }
        }

    fun setOidcChoice(value: OidcKeyChoice) =
        mutate(str(R.string.openid_provider_saved)) { repo.write { setKeyOidcChoice(id, value) } }

    /** Opens the provider's sign-in page in the browser and waits for it. */
    fun signIn() = mutate(null) {
        val s = repo.write { keyOidcSignIn(id) }
        _state.update { it.copy(notice = str(R.string.openid_signed_in_as, s.identity)) }
    }

    fun signOut() = mutate(str(R.string.openid_signed_out)) { repo.write { keyOidcSignOut(id) } }

    fun delete() {
        deleting = true
        _state.update { it.copy(working = true) }
        viewModelScope.launch {
            runCatching { repo.write { deleteKey(id) } }
                .onSuccess { _state.update { it.copy(working = false, deleted = true) } }
                .onFailure { e ->
                    deleting = false
                    _state.update { it.copy(working = false, notice = e.userMessage()) }
                }
        }
    }

    /** Returns the OpenSSH private key text or null after posting the error; never cached. */
    suspend fun exportPrivate(passphrase: String?, exportPassphrase: String?): String? =
        runCatching { repo.read { exportPrivateKey(id, passphrase, exportPassphrase) } }
            .onFailure { e -> _state.update { it.copy(notice = e.userMessage()) } }
            .getOrNull()
}
