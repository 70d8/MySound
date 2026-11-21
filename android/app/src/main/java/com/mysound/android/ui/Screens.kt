package com.mysound.android.ui

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.MoreVert
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalClipboardManager
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

/** แถวเพลง: แตะเพื่อเล่น, หัวใจ, เมนู ⋮ (เพิ่มเข้าเพลย์ลิสต์ / ให้คะแนน / แก้ชื่อ / ลบ) */
@Composable
private fun TrackRow(vm: MainViewModel, t: Track, list: List<Track>, playlistId: Long? = null) {
    var menu by remember { mutableStateOf(false) }
    var dialog by remember { mutableStateOf<String?>(null) }
    val now = vm.currentId == t.id
    Row(
        Modifier.fillMaxWidth().clickable { vm.play(t, list) }.padding(start = 16.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Art(t.title, 48.dp, cover = t.cover)
        Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
            Text(
                t.title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge,
                color = if (now) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurface,
            )
            val stars = if (t.rating > 0) " · ${"★".repeat(t.rating)}" else ""
            Text(
                "${t.artist} · ${fmt(t.durationMs)}$stars", maxLines = 1, overflow = TextOverflow.Ellipsis,
                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        IconButton(onClick = { vm.library.toggleFav(t.id) }) {
            Icon(
                if (t.fav) Icons.Filled.Favorite else Icons.Filled.FavoriteBorder,
                contentDescription = if (t.fav) tr("Unlike {0}", t.title) else tr("Like {0}", t.title),
                tint = if (t.fav) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Box {
            IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = tr("More actions for {0}", t.title)) }
            DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                DropdownMenuItem(text = { Text(tr("Add to playlist")) }, onClick = { menu = false; dialog = "playlist" })
                DropdownMenuItem(text = { Text(tr("Rate")) }, onClick = { menu = false; dialog = "rate" })
                DropdownMenuItem(text = { Text(tr("Edit info")) }, onClick = { menu = false; dialog = "rename" })
                if (playlistId != null && playlistId >= 0) {
                    DropdownMenuItem(text = { Text(tr("Remove from this playlist")) }, onClick = { menu = false; vm.playlists.removeTrack(playlistId, t.id) })
                }
                DropdownMenuItem(text = { Text(tr("Delete from library")) }, onClick = { menu = false; dialog = "delete" })
            }
        }
    }
    when (dialog) {
        "playlist" -> PlaylistPicker(vm, t.id) { dialog = null }
        "rate" -> RateDialog(t, onRate = { vm.rate(t.id, it) }) { dialog = null }
        "rename" -> RenameDialog(t, onDone = { title, artist -> vm.rename(t.id, title, artist); dialog = null }) { dialog = null }
        "delete" -> AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text(tr("Delete this song?")) },
            text = { Text(tr("“{0}” will be removed from your library. Files you imported from your phone are not deleted.", t.title)) },
            confirmButton = { TextButton(onClick = { dialog = null; vm.remove(t) }) { Text(tr("Delete")) } },
            dismissButton = { TextButton(onClick = { dialog = null }) { Text(tr("Cancel")) } },
        )
    }
}

@Composable
fun HomeScreen(vm: MainViewModel, onAdd: () -> Unit) {
    val recent = vm.library.tracks.sortedByDescending { it.addedAt }.take(20)
    val top = vm.library.tracks.filter { it.plays > 0 }.sortedByDescending { it.plays }.take(5)
    if (recent.isEmpty()) {
        Column(Modifier.fillMaxSize()) {
            Heading(tr("Recently added"))
            Empty(tr("Your library is empty."), tr("Add songs from this phone"), onAdd)
        }
        return
    }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 16.dp)) {
        item { Heading(tr("Recently added")) }
        items(recent, key = { "r${it.id}" }) { TrackRow(vm, it, recent) }
        if (top.isNotEmpty()) {
            item { Heading(tr("Most played")) }
            items(top, key = { "p${it.id}" }) { TrackRow(vm, it, top) }
        }
    }
}

@Composable
fun SongsScreen(vm: MainViewModel, onAdd: () -> Unit) {
    var query by rememberSaveable { mutableStateOf("") }
    var favOnly by rememberSaveable { mutableStateOf(false) }
    var sort by rememberSaveable { mutableStateOf(0) }
    val sortNames = listOf(tr("Recent"), tr("Title"), tr("Artist"), tr("Rating"))
    val all = vm.library.tracks
    val filtered = all.filter { (!favOnly || it.fav) && (query.isBlank() || "${it.title} ${it.artist} ${it.album}".contains(query, ignoreCase = true)) }
    val shown = when (sort) {
        1 -> filtered.sortedBy { it.title.lowercase() }
        2 -> filtered.sortedBy { it.artist.lowercase() }
        3 -> filtered.sortedByDescending { it.rating }
        else -> filtered.sortedByDescending { it.addedAt }
    }
    Box(Modifier.fillMaxSize()) {
        Column(Modifier.fillMaxSize()) {
            Heading(tr("All songs"))
            OutlinedTextField(
                value = query, onValueChange = { query = it }, singleLine = true,
                leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                placeholder = { Text(tr("Search your library…")) },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            )
            Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(selected = favOnly, onClick = { favOnly = !favOnly }, label = { Text(tr("Favorites")) })
                AssistChip(onClick = { sort = (sort + 1) % sortNames.size }, label = { Text(tr("Sort: {0}", sortNames[sort])) })
            }
            if (shown.isEmpty()) Empty(if (all.isEmpty()) tr("No songs yet.") else tr("No songs found."))
            else LazyColumn(contentPadding = PaddingValues(bottom = 96.dp)) { items(shown, key = { it.id }) { TrackRow(vm, it, shown) } }
        }
        ExtendedFloatingActionButton(
            onClick = onAdd,
            icon = { Icon(Icons.Filled.Add, contentDescription = null) },
            text = { Text(tr("Add songs")) },
            modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
        )
    }
}

private const val FAV = -1L

@Composable
fun PlaylistsScreen(vm: MainViewModel, backEnabled: Boolean) {
    var open by rememberSaveable { mutableStateOf<Long?>(null) }
    var creating by remember { mutableStateOf(false) }
    var renaming by remember { mutableStateOf(false) }
    var deleting by remember { mutableStateOf(false) }
    var menu by remember { mutableStateOf(false) }
    BackHandler(enabled = open != null && backEnabled) { open = null }

    val id = open
    val pl = vm.playlists.items.firstOrNull { it.id == id }
    LaunchedEffect(id, pl) { if (id != null && id != FAV && pl == null) open = null } // เพลย์ลิสต์ถูกลบไปแล้ว

    if (id == null) {
        val favs = vm.library.tracks.filter { it.fav }
        Box(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize()) {
                Heading(tr("Playlists"))
                LazyColumn(contentPadding = PaddingValues(bottom = 96.dp)) {
                    item {
                        PlaylistRow(tr("Favorites"), tr("{0} songs", favs.size), onClick = { open = FAV }, heart = true)
                    }
                    items(vm.playlists.items, key = { it.id }) { p ->
                        val n = p.trackIds.count { tid -> vm.library.tracks.any { it.id == tid } }
                        PlaylistRow(p.name, tr("{0} songs", n), onClick = { open = p.id })
                    }
                }
            }
            ExtendedFloatingActionButton(
                onClick = { creating = true },
                icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                text = { Text(tr("New playlist")) },
                modifier = Modifier.align(Alignment.BottomEnd).padding(16.dp),
            )
        }
        if (creating) TextDialog(tr("New playlist"), tr("Name"), "", tr("Create"), onDone = { vm.playlists.create(it); creating = false }, onDismiss = { creating = false })
        return
    }

    val tracks = if (id == FAV) vm.library.tracks.filter { it.fav } else pl?.trackIds.orEmpty().mapNotNull { tid -> vm.library.tracks.firstOrNull { it.id == tid } }
    val title = if (id == FAV) tr("Favorites") else pl?.name.orEmpty()
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(end = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconButton(onClick = { open = null }) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("Back to playlists")) }
            Text(title, Modifier.weight(1f), style = MaterialTheme.typography.headlineSmall, maxLines = 1, overflow = TextOverflow.Ellipsis)
            if (id != FAV) Box {
                IconButton(onClick = { menu = true }) { Icon(Icons.Filled.MoreVert, contentDescription = tr("Playlist options")) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    DropdownMenuItem(text = { Text(tr("Rename")) }, onClick = { menu = false; renaming = true })
                    DropdownMenuItem(text = { Text(tr("Delete playlist")) }, onClick = { menu = false; deleting = true })
                }
            }
        }
        if (tracks.isEmpty()) Empty(tr("No songs here yet. Use ⋮ on a song and choose “Add to playlist”."))
        else {
            Button(onClick = { vm.play(tracks.first(), tracks) }, modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
                Icon(Icons.Filled.PlayArrow, contentDescription = null)
                Text(tr("Play all"), Modifier.padding(start = 8.dp))
            }
            LazyColumn(contentPadding = PaddingValues(bottom = 16.dp)) { items(tracks, key = { it.id }) { TrackRow(vm, it, tracks, playlistId = id) } }
        }
    }
    if (renaming && pl != null) TextDialog(tr("Rename playlist"), tr("Name"), pl.name, tr("Save"), onDone = { vm.playlists.rename(pl.id, it); renaming = false }, onDismiss = { renaming = false })
    if (deleting && pl != null) AlertDialog(
        onDismissRequest = { deleting = false },
        title = { Text(tr("Delete playlist?")) },
        text = { Text(tr("The songs stay in your library.")) },
        confirmButton = { TextButton(onClick = { deleting = false; vm.playlists.delete(pl.id) }) { Text(tr("Delete")) } },
        dismissButton = { TextButton(onClick = { deleting = false }) { Text(tr("Cancel")) } },
    )
}

@Composable
private fun PlaylistRow(name: String, sub: String, onClick: () -> Unit, heart: Boolean = false) {
    Row(Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(Modifier.size(48.dp).clip(CircleShape).background(MaterialTheme.colorScheme.secondaryContainer), contentAlignment = Alignment.Center) {
            Icon(if (heart) Icons.Filled.Favorite else Icons.Filled.PlayArrow, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        }
        Column(Modifier.padding(start = 12.dp)) {
            Text(name, style = MaterialTheme.typography.bodyLarge, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(sub, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
fun DownloadScreen(vm: MainViewModel) {
    var url by rememberSaveable { mutableStateOf("") }
    var playlist by rememberSaveable { mutableStateOf(false) }
    val clipboard = LocalClipboardManager.current
    LaunchedEffect(vm.incomingUrl) { vm.incomingUrl?.let { url = it; vm.incomingUrl = null } }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Heading(tr("Download music"))
        OutlinedTextField(
            value = url, onValueChange = { url = it }, singleLine = true, enabled = !vm.dlBusy,
            label = { Text(tr("YouTube or SoundCloud link")) },
            placeholder = { Text("https://…") },
            trailingIcon = { TextButton(onClick = { clipboard.getText()?.text?.let { url = it.trim() } }, enabled = !vm.dlBusy) { Text(tr("Paste")) } },
            modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
        )
        Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            Switch(checked = playlist, onCheckedChange = { playlist = it }, enabled = !vm.dlBusy)
            Text(tr("Download the whole playlist"), Modifier.padding(start = 12.dp))
        }
        Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            if (vm.dlBusy) {
                Button(onClick = vm::cancelDownload) { Text(tr("Cancel")) }
            } else {
                Button(onClick = { vm.download(url, playlist) }, enabled = url.isNotBlank()) { Text(tr("Download")) }
                OutlinedButton(onClick = vm::updateEngine) { Text(tr("Update downloader")) }
            }
        }
        if (vm.dlBusy) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                if (vm.dlProgress in 0f..1f) LinearProgressIndicator(progress = { vm.dlProgress }, modifier = Modifier.fillMaxWidth())
                else LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                val pct = if (vm.dlProgress in 0f..1f) " ${(vm.dlProgress * 100).toInt()}%" else ""
                Text(vm.dlStatus + pct, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(tr("Keep the app open until the download finishes."), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        Text(
            tr("Tip: in the YouTube or SoundCloud app, tap Share and choose MySound to send the link here. Only download music you have the right to keep. If downloads suddenly stop working, tap “Update downloader”."),
            Modifier.padding(16.dp), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
fun SettingsScreen(vm: MainViewModel) {
    val pickImage = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri -> uri?.let(vm::setBackground) }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Heading(tr("Appearance"))

        Text(tr("Accent color"), Modifier.padding(horizontal = 16.dp), style = MaterialTheme.typography.titleMedium)
        Row(Modifier.padding(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Accents.forEach { (name, color) ->
                val selected = vm.accent == color.toArgb()
                Box(
                    Modifier.size(44.dp).clip(CircleShape).background(color)
                        .border(if (selected) 3.dp else 0.dp, MaterialTheme.colorScheme.onBackground, CircleShape)
                        .clickable(onClickLabel = tr("Use {0} accent", name)) { vm.chooseAccent(color.toArgb()) },
                )
            }
        }

        Text(tr("Language"), Modifier.padding(horizontal = 16.dp, vertical = 4.dp), style = MaterialTheme.typography.titleMedium)
        Row(Modifier.padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(selected = appLang == "en", onClick = { vm.chooseLang("en") }, label = { Text("English") })
            FilterChip(selected = appLang == "th", onClick = { vm.chooseLang("th") }, label = { Text("ไทย") })
        }

        Text(tr("Background image"), Modifier.padding(horizontal = 16.dp, vertical = 8.dp), style = MaterialTheme.typography.titleMedium)
        Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(onClick = { pickImage.launch(arrayOf("image/*")) }) { Text(tr("Choose image")) }
            if (vm.bgVersion > 0 && vm.bgFile.exists()) OutlinedButton(onClick = vm::clearBackground) { Text(tr("Remove")) }
        }
        Text(tr("Darken background: {0}%", (vm.bgDim * 100).toInt()), Modifier.padding(start = 16.dp, top = 12.dp), style = MaterialTheme.typography.bodyMedium)
        Slider(value = vm.bgDim, onValueChange = vm::chooseBgDim, valueRange = 0f..0.9f, modifier = Modifier.padding(horizontal = 16.dp))

        Text(tr("Background playback"), Modifier.padding(horizontal = 16.dp, vertical = 8.dp), style = MaterialTheme.typography.titleMedium)
        Text(
            tr("If music stops when the screen turns off, open Android Settings > Apps > MySound > Battery and choose “Unrestricted”. Some phones kill background apps to save power."),
            Modifier.padding(horizontal = 16.dp, vertical = 4.dp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(24.dp))
    }
}
