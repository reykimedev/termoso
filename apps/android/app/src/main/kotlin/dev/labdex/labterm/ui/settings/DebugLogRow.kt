package dev.labdex.labterm.ui.settings

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.text.format.DateUtils
import android.text.format.Formatter
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.BugReport
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.core.content.FileProvider
import androidx.core.content.getSystemService
import dev.labdex.labterm.R
import dev.labdex.labterm.core.DebugLogFile
import dev.labdex.labterm.data.DebugLog
import dev.labdex.labterm.data.userMessage
import dev.labdex.labterm.ui.components.ChevronRow
import dev.labdex.labterm.ui.components.IconTile
import dev.labdex.labterm.ui.components.RowDivider
import java.io.File

/** "Debug log" on debug builds; nothing on other builds. */
@Composable
fun DebugLogRow(onNotify: (String) -> Unit) {
    if (!DebugLog.enabled) return
    var open by remember { mutableStateOf(false) }
    RowDivider()
    ChevronRow(
        title = stringResource(R.string.debug_log),
        subtitle = stringResource(R.string.debug_log_hint),
        leading = { IconTile(Icons.Filled.BugReport) },
        modifier = Modifier.clickable { open = true },
    )
    if (open) DebugLogDialog(onNotify = onNotify, onDismiss = { open = false })
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DebugLogDialog(onNotify: (String) -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    var files by remember { mutableStateOf<List<DebugLogFile>>(emptyList()) }
    // A file name, or DebugLog.LOGCAT; null until the file list is in.
    var source by remember { mutableStateOf<String?>(null) }
    var lines by remember { mutableStateOf<List<String>?>(null) }
    var revision by remember { mutableIntStateOf(0) }
    val copied = stringResource(R.string.copied_2)
    val listState = rememberLazyListState()
    LaunchedEffect(revision) {
        files = DebugLog.files()
        // Start on this launch's file; keep the pick across refreshes while it exists.
        source = source?.takeIf { s -> s == DebugLog.LOGCAT || files.any { it.name == s } }
            ?: files.firstOrNull { it.current }?.name
            ?: files.firstOrNull()?.name
            ?: DebugLog.LOGCAT
    }
    LaunchedEffect(source, revision) {
        val s = source ?: return@LaunchedEffect
        lines = null
        lines = runCatching { DebugLog.read(s) }.getOrElse { listOf(it.userMessage()) }
    }
    LaunchedEffect(lines) {
        lines?.let { if (it.isNotEmpty()) listState.scrollToItem(it.size - 1) }
    }
    Dialog(onDismissRequest = onDismiss, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = { Text(stringResource(R.string.debug_log)) },
                    navigationIcon = {
                        IconButton(onClick = onDismiss) {
                            Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close))
                        }
                    },
                    actions = {
                        IconButton(onClick = { revision++ }) {
                            Icon(Icons.Filled.Refresh, contentDescription = stringResource(R.string.refresh))
                        }
                        IconButton(onClick = {
                            val text = lines?.joinToString("\n").orEmpty()
                            context.getSystemService<ClipboardManager>()
                                ?.setPrimaryClip(ClipData.newPlainText("debug log", text))
                            onNotify(copied)
                        }) {
                            Icon(Icons.Filled.ContentCopy, contentDescription = stringResource(R.string.copy))
                        }
                        IconButton(onClick = {
                            runCatching { shareLog(context, source, lines.orEmpty()) }
                                .onFailure { onNotify(it.userMessage()) }
                        }) {
                            Icon(Icons.Filled.Share, contentDescription = stringResource(R.string.share))
                        }
                    },
                )
            },
        ) { padding ->
            Column(Modifier.fillMaxSize().padding(padding)) {
                SourcePicker(
                    files = files,
                    selected = source,
                    onPick = { source = it },
                    modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
                )
                val l = lines
                when {
                    l == null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator()
                    }
                    l.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        Text(
                            stringResource(R.string.debug_log_empty),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    else -> LazyColumn(
                        state = listState,
                        modifier = Modifier.fillMaxSize().padding(8.dp),
                    ) {
                        items(l) { line ->
                            Text(
                                line,
                                fontFamily = FontFamily.Monospace,
                                fontSize = 11.sp,
                                color = lineColor(line),
                            )
                        }
                    }
                }
            }
        }
    }
}

/** Which log to show: this launch's file, earlier launches, or live logcat. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SourcePicker(
    files: List<DebugLogFile>,
    selected: String?,
    onPick: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val context = LocalContext.current
    val options = files.map { it.name to fileLabel(context, it) } +
        (DebugLog.LOGCAT to stringResource(R.string.debug_log_logcat))
    var open by remember { mutableStateOf(false) }
    ExposedDropdownMenuBox(expanded = open, onExpandedChange = { open = it }, modifier = modifier) {
        OutlinedTextField(
            value = options.firstOrNull { it.first == selected }?.second.orEmpty(),
            onValueChange = {},
            readOnly = true,
            label = { Text(stringResource(R.string.debug_log_source)) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = open) },
            modifier = Modifier
                .fillMaxWidth()
                .menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable),
            singleLine = true,
        )
        ExposedDropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            options.forEach { (id, label) ->
                DropdownMenuItem(
                    text = { Text(label) },
                    onClick = {
                        onPick(id)
                        open = false
                    },
                    contentPadding = ExposedDropdownMenuDefaults.ItemContentPadding,
                )
            }
        }
    }
}

private fun fileLabel(context: Context, f: DebugLogFile): String {
    val size = Formatter.formatShortFileSize(context, f.size.toLong())
    if (f.current) return context.getString(R.string.debug_log_this_launch, size)
    val time = DateUtils.formatDateTime(
        context,
        f.modified * 1000,
        DateUtils.FORMAT_SHOW_DATE or DateUtils.FORMAT_SHOW_TIME or DateUtils.FORMAT_ABBREV_MONTH,
    )
    return context.getString(R.string.debug_log_earlier_launch, time, size)
}

@Composable
private fun lineColor(line: String) = when {
    " E/" in line || " F/" in line || " ERROR " in line -> MaterialTheme.colorScheme.error
    " W/" in line || " WARN " in line -> MaterialTheme.colorScheme.tertiary
    else -> MaterialTheme.colorScheme.onSurface
}

// A file, not EXTRA_TEXT: a few thousand lines can exceed the binder transaction limit.
private fun shareLog(context: Context, source: String?, lines: List<String>) {
    val dir = File(context.cacheDir, "logs").apply { mkdirs() }
    val name = source?.takeIf { it != DebugLog.LOGCAT }?.removeSuffix(".log") ?: "labterm-logcat"
    val file = File(dir, "$name.txt").apply { writeText(lines.joinToString("\n")) }
    val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file)
    val send = Intent(Intent.ACTION_SEND)
        .setType("text/plain")
        .putExtra(Intent.EXTRA_STREAM, uri)
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    context.startActivity(
        Intent.createChooser(send, context.getString(R.string.share)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
    )
}
