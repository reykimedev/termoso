package dev.labdex.labterm.ui

import android.security.keystore.UserNotAuthenticatedException
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.LocalViewModelStoreOwner
import dev.labdex.labterm.R
import dev.labdex.labterm.ui.security.PinLockScreen
import dev.labdex.labterm.data.AppContainer
import dev.labdex.labterm.data.VaultState
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.core.Fido2Listener
import dev.labdex.labterm.core.unlockWithSecurityKey
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.account.AuthMode
import dev.labdex.labterm.ui.account.SignInScreen
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.security.AuthResult
import dev.labdex.labterm.ui.security.SecurityKeyWait
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import dev.labdex.labterm.ui.security.authenticateDevice
import dev.labdex.labterm.ui.security.findFragmentActivity
import dev.labdex.labterm.ui.shell.MainShell
import dev.labdex.labterm.ui.welcome.WelcomeScreen
import kotlinx.coroutines.launch

/**
 * Entry composable. Opens the vault on launch, shows the welcome screen once,
 * then the tabbed shell. With app lock on, the Keystore refuses to unwrap the
 * master key until the user passes the system prompt; returning from the
 * background after the configured delay covers the shell with the same prompt
 * while sessions keep running. An explicit lock from Settings closes the store
 * until the user taps Unlock. With security keys enrolled the lock screen also
 * waits for a key (USB or NFC); without app lock it is the only way in.
 */
@Composable
fun LabtermRoot(container: AppContainer, vault: VaultState) {
    val context = LocalContext.current
    var error by remember { mutableStateOf<String?>(null) }
    var attempt by remember { mutableIntStateOf(0) }
    var manualLock by remember { mutableStateOf(false) }
    var needAuth by remember { mutableStateOf(false) }
    var welcomeAuth by remember { mutableStateOf<AuthMode?>(null) }
    val securityKeys by container.securityKeys.collectAsStateWithLifecycle()
    val gated by container.gated.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()

    suspend fun prompt(): Boolean {
        val activity = context.findFragmentActivity() ?: return false
        return when (val r = authenticateDevice(activity, str(R.string.unlock_labterm), str(R.string.use_your_fingerprint_or_screen_lock))) {
            AuthResult.Success -> true
            AuthResult.Cancelled -> false
            is AuthResult.Failed -> {
                error = r.message
                false
            }
        }
    }

    // Try the Keystore first and prompt only when it asks for the user: one
    // successful prompt opens the vault within the auth window. (Prompting
    // up front on every retry showed the system prompt twice on launch.)
    LaunchedEffect(attempt, manualLock) {
        if (vault !is VaultState.Locked || manualLock) return@LaunchedEffect
        var result = runCatching { container.unlock() }
        if (result.exceptionOrNull() is AppContainer.SecurityKeyRequired) {
            // Only a key opens it: the lock screen waits for one.
            needAuth = true
            return@LaunchedEffect
        }
        if (result.exceptionOrNull() is UserNotAuthenticatedException) {
            needAuth = true
            if (!prompt()) return@LaunchedEffect
            result = runCatching { container.unlock() }
        }
        result
            .onSuccess { needAuth = false }
            .onFailure { error = it.userMessage() }
    }
    LaunchedEffect(gated) {
        if (gated && !container.appPin.isSet && container.masterKeys.authRequired() && prompt()) container.ungate()
    }

    when (vault) {
        is VaultState.Locked -> LockedScreen(
            error = error,
            manual = manualLock || needAuth,
            authRequired = container.masterKeys.authRequired(),
            securityKey = securityKeys && (manualLock || needAuth),
            onSecurityKey = { deviceId, listener ->
                container.unlockWithSecurityKey(deviceId, listener)
                manualLock = false
                needAuth = false
                error = null
            },
            onRetry = { error = null; manualLock = false; attempt++ },
        )
        is VaultState.Open -> {
            val settings by vault.repo.settings.collectAsStateWithLifecycle()
            val session = remember(vault.repo) { SessionStoreOwner() }
            DisposableEffect(session) { onDispose { session.viewModelStore.clear() } }
            val restoring by vault.account.restoring.collectAsStateWithLifecycle()
            val accountStatus by vault.account.status.collectAsStateWithLifecycle()
            fun finishWelcome() {
                welcomeAuth = null
                scope.launch { vault.repo.updateSettings { it.copy(welcomeSeen = true) } }
            }
            // The re-auth gate only covers the UI: the vault stays open, so
            // navigation, the selected vault and the terminals underneath
            // survive it.
            Box {
                CompositionLocalProvider(LocalViewModelStoreOwner provides session) {
                    when {
                        // A returning user who is still signed in never sees the welcome again.
                        !settings.welcomeSeen && restoring -> LockedScreen(error = null, manual = false, authRequired = false, securityKey = false, onSecurityKey = { _, _ -> }, onRetry = {})
                        !settings.welcomeSeen && accountStatus.account != null -> LaunchedEffect(Unit) { finishWelcome() }
                        !settings.welcomeSeen && welcomeAuth != null -> SignInScreen(
                            container = container,
                            account = vault.account,
                            mode = welcomeAuth ?: AuthMode.SignIn,
                            onBack = { welcomeAuth = null },
                            onDone = ::finishWelcome,
                        )
                        !settings.welcomeSeen -> WelcomeScreen(
                            onCreateAccount = { welcomeAuth = AuthMode.Register },
                            onSignIn = { welcomeAuth = AuthMode.SignIn },
                            onContinueOffline = ::finishWelcome,
                        )
                        else -> MainShell(
                            container = container,
                            repo = vault.repo,
                            sessions = vault.sessions,
                            sftp = vault.sftp,
                            forwards = vault.forwards,
                            account = vault.account,
                            presence = vault.presence,
                            ai = vault.ai,
                            onLock = {
                                manualLock = true
                                scope.launch { container.lockVault() }
                            },
                        )
                    }
                }
                if (gated && container.appPin.isSet) {
                    PinLockScreen(container.appPin, onUnlocked = { container.ungate() })
                } else if (gated) {
                    val authRequired = container.masterKeys.authRequired()
                    LockedScreen(
                        error = error,
                        manual = true,
                        authRequired = authRequired,
                        securityKey = securityKeys,
                        onSecurityKey = { deviceId, listener ->
                            // The vault is open; the key only proves who is here.
                            withContext(Dispatchers.IO) {
                                unlockWithSecurityKey(container.profileDir.absolutePath, deviceId, listener).fill(0)
                            }
                            container.ungate()
                        },
                        onRetry = if (authRequired) {
                            {
                                error = null
                                scope.launch { if (prompt()) container.ungate() }
                            }
                        } else {
                            null
                        },
                    )
                }
            }
        }
    }
}

/** ViewModel scope bound to one opened vault, so nothing outlives a lock. */
private class SessionStoreOwner : ViewModelStoreOwner {
    override val viewModelStore = ViewModelStore()
}

@Composable
private fun LockedScreen(
    error: String?,
    manual: Boolean,
    authRequired: Boolean,
    securityKey: Boolean,
    onSecurityKey: suspend (deviceId: String, listener: Fido2Listener) -> Unit,
    onRetry: (() -> Unit)?,
) {
    Scaffold { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            when {
                manual -> {
                    IconTile(Icons.Filled.Lock, size = 64, tint = MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.height(16.dp))
                    Text(stringResource(R.string.labterm_is_locked), style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
                    Text(
                        error ?: when {
                            authRequired -> stringResource(R.string.unlock_with_your_fingerprint_or_screen_lock_to)
                            securityKey -> stringResource(R.string.use_your_security_key_to_open_labterm)
                            else -> stringResource(R.string.the_encrypted_vault_is_closed_unlock_to_continue)
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = if (error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                    Spacer(Modifier.height(16.dp))
                    // Without app lock and with keys, Unlock would only fail again.
                    if (onRetry != null && (authRequired || !securityKey)) {
                        Button(onClick = onRetry) { Text(stringResource(R.string.unlock)) }
                    }
                    if (securityKey) {
                        Spacer(Modifier.height(24.dp))
                        SecurityKeyWait(title = stringResource(R.string.unlock_with_security_key), attempt = onSecurityKey)
                    }
                }
                error == null -> CircularProgressIndicator()
                else -> {
                    Text(stringResource(R.string.could_not_open_the_vault), style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
                    Text(error, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error, textAlign = TextAlign.Center)
                    Spacer(Modifier.height(16.dp))
                    if (onRetry != null) Button(onClick = onRetry) { Text(stringResource(R.string.retry)) }
                }
            }
        }
    }
}

