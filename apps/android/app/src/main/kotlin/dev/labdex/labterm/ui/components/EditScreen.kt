package dev.labdex.labterm.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import dev.labdex.labterm.R

/**
 * The one layout of every full-screen edit page (hosts, groups, tags, keys,
 * identities, proxies, chains): close on the left, Save on the right, cards
 * below and [DeleteCard] last.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EditScreen(
    title: String,
    canSave: Boolean,
    onSave: () -> Unit,
    onClose: () -> Unit,
    saving: Boolean = false,
    content: @Composable (PaddingValues) -> Unit,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(title) },
                navigationIcon = {
                    IconButton(onClick = onClose) { Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close)) }
                },
                actions = {
                    if (saving) {
                        CircularProgressIndicator(Modifier.padding(end = 16.dp).size(20.dp), strokeWidth = 2.dp)
                    } else {
                        TextButton(onClick = onSave, enabled = canSave) { Text(stringResource(R.string.save)) }
                    }
                },
            )
        },
        content = content,
    )
}

/** The red delete row that ends an edit page. */
@Composable
fun DeleteCard(label: String, onClick: () -> Unit, subtitle: String? = null) {
    SectionCard {
        ListRow(
            title = label,
            subtitle = subtitle,
            titleColor = MaterialTheme.colorScheme.error,
            modifier = Modifier.clickable(onClick = onClick),
        )
    }
}
