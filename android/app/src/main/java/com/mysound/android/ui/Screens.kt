package com.mysound.android.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.mysound.android.MainViewModel
import com.mysound.android.data.Track
import com.mysound.android.data.fmt

@Composable
private fun Heading(text: String) =
    Text(text, style = MaterialTheme.typography.headlineMedium, modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp))

@Composable
private fun Empty(text: String, action: String? = null, onAction: () -> Unit = {}) {
    Column(Modifier.fillMaxWidth().padding(32.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant)
        if (action != null) Button(onClick = onAction) { Text(action) }
    }
}

@Composable
private fun TrackRow(vm: MainViewModel, t: Track, list: List<Track>) {
    var menu by remember { mutableStateOf(false) }
    val now = vm.currentId == t.id
    Row(
        Modifier.fillMaxWidth().clickable { vm.play(t, list) }.padding(start = 16.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Art(t.title, 48.dp)
        Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
            Text(
                t.title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge,
                color = if (now) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
            )
            Text(
                "${t.artist} · ${fmt(t.durationMs)}", maxLines = 1, overflow = TextOverflow.Ellipsis,
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        IconButton(onClick = { vm.library.toggleFav(t.id) }) {
            Icon(
                if (t.fav) Icons.Filled.Favorite else Icons.Filled.FavoriteBorder,
                contentDescription = if (t.fav) "Unlike ${t.title}" else "Like ${t.title}",
                tint = if (t.fav) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = "More actions for ${t.title}") }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(text = { Text("Remove from library") }, onClick = { menu = false; vm.remove(t) })
            }
        }
    }
}

@Composable
fun HomeScreen(vm: MainViewModel, onAdd: () -> Unit) {
    val recent = vm.library.tracks.sortedByDescending { it.addedAt }.take(20)
    Column(Modifier.fillMaxSize()) {
        Heading("Recently added")
        if (recent.isEmpty()) Empty("Your library is empty.", "Add songs from this phone", onAdd)
        else LazyColumn(contentPadding = PaddingValues(bottom = 16.dp)) { items(recent, key = { it.id }) { TrackRow(vm, it, recent) } }
    }
}

@Composable
fun SongsScreen(vm: MainViewModel, onAdd: () -> Unit) {
    var query by remember { mutableStateOf("") }
    val all = vm.library.tracks
    val shown = if (query.isBlank()) all else all.filter { "${it.title} ${it.artist} ${it.album}".contains(query, ignoreCase = true) }
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            Heading("All songs")
            OutlinedTextField(
                value = query, onValueChange = { query = it }, singleLine = true,
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                placeholder = { Text("Search your library…") },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            )
            if (shown.isEmpty()) Empty(if (all.isEmpty()) "No songs yet." else "No songs found.")
            else LazyColumn(contentPadding = PaddingValues(top = 8.dp, bottom = 96.dp)) { items(shown, key = { it.id }) { TrackRow(vm, it, shown) } }
        }
        ExtendedFloatingActionButton(
            onClick = onAdd,
            icon = { Icon(Icons.Filled.Add, contentDescription = null) },
            text = { Text("Add songs") },
            modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
        )
    }
}

@Composable
fun DownloadScreen() {
    Column(Modifier.fillMaxSize()) {
        Heading("Download music")
        Card(Modifier.padding(16.dp).fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Coming in the next update", style = MaterialTheme.typography.titleMedium)
                Text(
                    "Paste a YouTube or SoundCloud link here and the song will be downloaded straight to this phone. For now, use “Add songs” to import audio files you already have.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
fun SettingsScreen(vm: MainViewModel) {
    Column(Modifier.fillMaxSize()) {
        Heading("Appearance")
        Text("Accent color", Modifier.padding(horizontal = 16.dp), style = MaterialTheme.typography.titleMedium)
        Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Accents.forEach { (name, color) ->
                val selected = vm.accent == color.toArgb()
                Box(
                    Modifier.size(44.dp).clip(CircleShape).background(color)
                        .border(if (selected) 3.dp else 0.dp, MaterialTheme.colorScheme.onBackground, CircleShape)
                        .clickable(onClickLabel = "Use $name accent") { vm.chooseAccent(color.toArgb()) },
                )
            }
        }
        Text("Background playback", Modifier.padding(horizontal = 16.dp, vertical = 8.dp), style = MaterialTheme.typography.titleMedium)
        Text(
            "If music stops when the screen turns off, open Android Settings > Apps > MySound > Battery and choose “Unrestricted”. Some phones kill background apps to save power.",
            Modifier.padding(horizontal = 16.dp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
