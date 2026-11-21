package com.mysound.android.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.mysound.android.MainViewModel
import com.mysound.android.data.Track

@Composable
fun TextDialog(title: String, label: String, initial: String, confirm: String, onDone: (String) -> Unit, onDismiss: () -> Unit) {
    var v by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        text = { OutlinedTextField(value = v, onValueChange = { v = it }, singleLine = true, label = { Text(label) }) },
        confirmButton = { TextButton(onClick = { if (v.isNotBlank()) onDone(v.trim()) }) { Text(confirm) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("Cancel")) } },
    )
}

@Composable
fun RenameDialog(t: Track, onDone: (String, String) -> Unit, onDismiss: () -> Unit) {
    var title by remember { mutableStateOf(t.title) }
    var artist by remember { mutableStateOf(t.artist) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("Edit song info")) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(value = title, onValueChange = { title = it }, singleLine = true, label = { Text(tr("Title")) })
                OutlinedTextField(value = artist, onValueChange = { artist = it }, singleLine = true, label = { Text(tr("Artist")) })
            }
        },
        confirmButton = { TextButton(onClick = { if (title.isNotBlank()) onDone(title.trim(), artist.trim().ifEmpty { "Unknown artist" }) }) { Text(tr("Save")) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(tr("Cancel")) } },
    )
}

@Composable
fun StarRow(rating: Int, size: Dp = 32.dp, onRate: (Int) -> Unit) {
    Row {
        for (n in 1..5) {
            IconButton(onClick = { onRate(n) }, modifier = Modifier.size(size + 8.dp)) {
                Icon(
                    if (n <= rating) Icons.Filled.Star else Icons.Filled.StarBorder,
                    contentDescription = tr("{0} stars", n),
                    tint = if (n <= rating) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.size(size),
                )
            }
        }
    }
}

@Composable
fun RateDialog(t: Track, onRate: (Int) -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("Rate this song")) },
        text = { StarRow(t.rating) { onRate(it); onDismiss() } },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("Close")) } },
    )
}

@Composable
fun PlaylistPicker(vm: MainViewModel, trackId: Long, onDismiss: () -> Unit) {
    var creating by remember { mutableStateOf(false) }
    if (creating) {
        TextDialog(
            title = tr("New playlist"), label = tr("Name"), initial = "", confirm = tr("Create"),
            onDone = { name ->
                val id = vm.playlists.create(name)
                vm.playlists.addTrack(id, trackId)
                vm.message = tr("Added to {0}", name)
                onDismiss()
            },
            onDismiss = { creating = false },
        )
        return
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("Add to playlist")) },
        text = {
            Column(Modifier.fillMaxWidth()) {
                vm.playlists.items.forEach { p ->
                    TextButton(
                        onClick = { vm.playlists.addTrack(p.id, trackId); vm.message = tr("Added to {0}", p.name); onDismiss() },
                        modifier = Modifier.fillMaxWidth(),
                    ) { Text(p.name, modifier = Modifier.fillMaxWidth()) }
                }
                TextButton(onClick = { creating = true }, modifier = Modifier.fillMaxWidth()) { Text(tr("+ New playlist"), modifier = Modifier.fillMaxWidth()) }
            }
        },
        confirmButton = { TextButton(onClick = onDismiss) { Text(tr("Close")) } },
    )
}
