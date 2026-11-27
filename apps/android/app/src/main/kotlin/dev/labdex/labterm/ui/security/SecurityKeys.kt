package dev.labdex.labterm.ui.security

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Key
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import dev.labdex.labterm.R
import dev.labdex.labterm.core.Fido2Listener
import dev.labdex.labterm.core.MobileException
import dev.labdex.labterm.core.UnlockKeyCard
import dev.labdex.labterm.data.AppContainer
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.str
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.ListRow
import dev.labdex.labterm.ui.components.RowDivider
import dev.labdex.labterm.ui.components.SectionCard
import dev.labdex.labterm.ui.keychain.LocalFido2
import dev.labdex.labterm.ui.keychain.SecurityKeyListening
import dev.labdex.labterm.ui.keychain.waitingHint
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** What to tell the person when a security key unlock or enrolment failed; `null` for a cancel. */
fun securityKeyError(e: Throwable): String? = when {
    e is MobileException.Cancelled -> null
    e is MobileException.SecurityKey && e.kind == "fido2_wrong_device" -> str(R.string.security_key_not_set_up_for_labterm)
    else -> e.userMessage()
}

private fun Throwable.isPinRequired(): Boolean =
    this is MobileException.SecurityKey && kind == "fido2_pin_required"

/**
 * Waits for a security key (USB or NFC) and runs [attempt] with it as soon as
 * one is attached; a failed attempt can be retried by tapping again. Used by
 * the lock screen and by confirmations in Settings.
 */
@Composable
fun SecurityKeyWait(
    title: String,
    attempt: suspend (deviceId: String, listener: Fido2Listener) -> Unit,
    modifier: Modifier = Modifier,
) {
    val fido2 = LocalFido2.current
    SecurityKeyListening(fido2)
    val devices by fido2.devices.collectAsStateWithLifecycle()
    val pending by fido2.usbPending.collectAsStateWithLifecycle()
    var busy by remember { mutableStateOf(false) }
    var touch by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var tried by remember { mutableStateOf<String?>(null) }
    var retry by remember { mutableIntStateOf(0) }
    val device = devices.firstOrNull()
    // One attempt per attached key: holding the same key in place does not loop.
    LaunchedEffect(device?.id, retry) {
        val id = device?.id ?: run { tried = null; return@LaunchedEffect }
        if (busy || (tried == id && retry == 0)) return@LaunchedEffect
        tried = id
        busy = true
        touch = false
        error = null
        val listener = object : Fido2Listener {
            override fun onTouch() {
                touch = true
            }
        }
        runCatching { attempt(id, listener) }
            .onFailure { error = securityKeyError(it) }
        busy = false
        touch = false
        retry = 0
    }
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        Text(title, style = MaterialTheme.typography.titleSmall, textAlign = TextAlign.Center)
        Spacer(Modifier.height(8.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            if (busy || device == null) CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
            Text(
                when {
                    busy && touch -> stringResource(R.string.touch_your_security_key)
                    busy -> stringResource(R.string.talking_to_the_security_key)
                    device == null && pending > 0 -> stringResource(R.string.allow_labterm_to_use_the_usb_device_in)
                    device == null -> fido2.waitingHint()
                    else -> device.product.ifBlank { stringResource(R.string.security_key) }
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
        }
        error?.let {
            Spacer(Modifier.height(8.dp))
            Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error, textAlign = TextAlign.Center)
            if (device != null && !busy) {
                TextButton(onClick = { retry++ }) { Text(stringResource(R.string.try_again)) }
            }
        }
    }
}

/**
 * Settings → Security → Security keys: the enrolled keys, add one (name, then
 * the key; its PIN only when it asks), remove one. With app lock on, adding or
 * removing a key is confirmed with the device authentication first; removing
 * the last key while app lock is off is confirmed with that key.
 */
@Composable
fun SecurityKeysDialog(container: AppContainer, notify: (String) -> Unit, onDismiss: () -> Unit) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val scope = rememberCoroutineScope()
    var keys by remember { mutableStateOf<List<UnlockKeyCard>>(emptyList()) }
    var reload by remember { mutableIntStateOf(0) }
    LaunchedEffect(reload) {
        keys = runCatching { withContext(Dispatchers.IO) { container.securityKeyList() } }.getOrDefault(emptyList())
    }
    var adding by remember { mutableStateOf(false) }
    var removing by remember { mutableStateOf<UnlockKeyCard?>(null) }
    val appLock by container.appLock.collectAsStateWithLifecycle()

    /** Device authentication when app lock is on; nothing to confirm otherwise. */
    suspend fun confirmOwner(title: String): Boolean {
        if (!container.masterKeys.authRequired()) return true
        val activity = context.findFragmentActivity() ?: return false
        return when (val r = authenticateDevice(activity, title, str(R.string.confirm_its_you))) {
            AuthResult.Success -> true
            AuthResult.Cancelled -> false
            is AuthResult.Failed -> {
                notify(r.message)
                false
            }
        }
    }

    if (adding) {
        AddSecurityKeyDialog(
            container = container,
            defaultName = if (keys.isEmpty()) str(R.string.security_key) else str(R.string.backup_security_key_n, keys.size),
            onDone = { card ->
                adding = false
                reload++
                notify(str(R.string.security_key_added, card.name))
            },
            onDismiss = { adding = false },
        )
    }
    removing?.let { key ->
        val lastWayIn = keys.size == 1 && !appLock
        AlertDialog(
            onDismissRequest = { removing = null },
            title = { Text(stringResource(R.string.remove_security_key)) },
            text = {
                Column {
                    Text(
                        if (lastWayIn) {
                            stringResource(R.string.remove_last_security_key_hint)
                        } else {
                            stringResource(R.string.remove_security_key_hint, key.name)
                        },
                    )
                    if (lastWayIn) {
                        Spacer(Modifier.height(16.dp))
                        SecurityKeyWait(
                            title = stringResource(R.string.confirm_with_this_security_key),
                            attempt = { deviceId, listener ->
                                withContext(Dispatchers.IO) {
                                    // Proves the key at hand is an enrolled one before the
                                    // vault goes back to opening without anyone.
                                    dev.labdex.labterm.core.unlockWithSecurityKey(container.profileDir.absolutePath, deviceId, listener).fill(0)
                                    container.removeSecurityKey(key.id)
                                }
                                removing = null
                                reload++
                                notify(str(R.string.security_key_removed))
                            },
                        )
                    }
                }
            },
            confirmButton = {
                if (!lastWayIn) {
                    TextButton(onClick = {
                        scope.launch {
                            if (!confirmOwner(str(R.string.remove_security_key))) return@launch
                            runCatching { withContext(Dispatchers.IO) { container.removeSecurityKey(key.id) } }
                                .onSuccess { notify(str(R.string.security_key_removed)) }
                                .onFailure { notify(it.userMessage()) }
                            removing = null
                            reload++
                        }
                    }) { Text(stringResource(R.string.remove_2)) }
                }
            },
            dismissButton = { TextButton(onClick = { removing = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.security_keys)) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                Text(
                    stringResource(R.string.security_keys_unlock_hint),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(12.dp))
                if (keys.isNotEmpty()) {
                    SectionCard {
                        keys.forEachIndexed { i, k ->
                            if (i > 0) RowDivider()
                            ListRow(
                                title = k.name,
                                subtitle = keySubtitle(k),
                                leading = { IconTile(Icons.Filled.Key) },
                                trailing = {
                                    TextButton(onClick = { removing = k }) { Text(stringResource(R.string.remove_2)) }
                                },
                            )
                        }
                    }
                    if (keys.size == 1 && !appLock) {
                        Spacer(Modifier.height(8.dp))
                        Text(
                            stringResource(R.string.add_a_backup_security_key_hint),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = {
                scope.launch { if (confirmOwner(str(R.string.add_a_security_key))) adding = true }
            }) { Text(stringResource(R.string.add_a_security_key)) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.done)) } },
    )
}

private fun keySubtitle(k: UnlockKeyCard): String {
    val fmt = DateFormat.getDateInstance(DateFormat.MEDIUM)
    val added = str(R.string.added_on, fmt.format(Date(k.createdAt.toLong() * 1000)))
    val used = k.lastUsedAt?.let { str(R.string.last_used_on, fmt.format(Date(it.toLong() * 1000))) }
    return listOfNotNull(added, used).joinToString(" · ")
}

/** Name the key, then plug it in or hold it to the phone; a PIN field appears only if the key asks. */
@Composable
private fun AddSecurityKeyDialog(
    container: AppContainer,
    defaultName: String,
    onDone: (UnlockKeyCard) -> Unit,
    onDismiss: () -> Unit,
) {
    var name by remember { mutableStateOf(defaultName) }
    var pin by remember { mutableStateOf("") }
    var pinNeeded by remember { mutableStateOf(false) }
    var started by remember { mutableStateOf(false) }
    var attempt by remember { mutableIntStateOf(0) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.add_a_security_key)) },
        text = {
            Column {
                Text(
                    stringResource(R.string.add_security_key_unlock_hint),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it.take(64) },
                    label = { Text(stringResource(R.string.name)) },
                    singleLine = true,
                    enabled = !started,
                    modifier = Modifier.fillMaxWidth(),
                )
                if (pinNeeded) {
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(
                        value = pin,
                        onValueChange = { pin = it },
                        label = { Text(stringResource(R.string.security_key_pin)) },
                        supportingText = { Text(stringResource(R.string.security_key_pin_only_now)) },
                        singleLine = true,
                        enabled = !started,
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (started) {
                    Spacer(Modifier.height(16.dp))
                    // Keyed on the attempt so a PIN retry starts a fresh wait.
                    androidx.compose.runtime.key(attempt) {
                        SecurityKeyWait(
                            title = stringResource(R.string.plug_in_or_hold_the_new_key_touch_twice),
                            attempt = { deviceId, listener ->
                                try {
                                    val card = withContext(Dispatchers.IO) {
                                        container.addSecurityKey(deviceId, name.trim(), pin.ifEmpty { null }, listener)
                                    }
                                    onDone(card)
                                } catch (e: Throwable) {
                                    if (e.isPinRequired()) {
                                        pinNeeded = true
                                        started = false
                                    } else {
                                        throw e
                                    }
                                }
                            },
                        )
                    }
                }
            }
        },
        confirmButton = {
            if (!started) {
                TextButton(
                    enabled = name.isNotBlank() && (!pinNeeded || pin.length >= 4),
                    onClick = { started = true; attempt++ },
                ) { Text(stringResource(R.string.continue_)) }
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.cancel)) } },
    )
}
