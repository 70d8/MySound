package com.mysound.android.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.Brush
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Lyrics
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.Shuffle
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.SkipPrevious
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.mysound.android.MainViewModel
import com.mysound.android.data.Track
import com.mysound.android.data.fmt

/** ปุ่มเล่น/หยุดวงกลมสีตัวอักษร ไอคอนสีพื้น เหมือนบนคอม */
@Composable
private fun playColors() = IconButtonDefaults.filledIconButtonColors(
    containerColor = MaterialTheme.colorScheme.onBackground,
    contentColor = MaterialTheme.colorScheme.background,
)

/** แถบเล่นเพลงเล็กเหนือแท็บล่าง: แตะเพื่อขยายเต็มจอ */
@Composable
fun MiniPlayer(vm: MainViewModel, t: Track, onOpen: () -> Unit) {
    Surface(
        modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp),
        shape = RoundedCornerShape(LocalLook.current.layout.radius.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
    ) {
        Column {
            LinearProgressIndicator(
                progress = { if (vm.durationMs > 0) (vm.positionMs.toFloat() / vm.durationMs).coerceIn(0f, 1f) else 0f },
                modifier = Modifier.fillMaxWidth().height(2.dp),
            )
            Row(
                Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(horizontal = 12.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Art(t.title, 44.dp, cover = t.cover)
                Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                    Text(t.title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge)
                    Text(t.artist, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                IconButton(onClick = vm::prev) { Icon(Icons.Filled.SkipPrevious, contentDescription = tr("Previous")) }
                FilledIconButton(onClick = vm::toggle, modifier = Modifier.size(40.dp), colors = playColors()) {
                    Icon(if (vm.isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow, contentDescription = if (vm.isPlaying) tr("Pause") else tr("Play"))
                }
                IconButton(onClick = vm::next) { Icon(Icons.Filled.SkipNext, contentDescription = tr("Next")) }
            }
        }
    }
}

/** หน้าเล่นเพลงเต็มจอ */
@Composable
fun NowPlaying(vm: MainViewModel, t: Track, onClose: () -> Unit) {
    var lyricsOn by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(t.id, lyricsOn) { if (lyricsOn) vm.loadLyrics(t) }

    val tint = rememberTint(t.title, t.cover)
    val base = MaterialTheme.colorScheme.background
    val brush = if (LocalLook.current.tint) Brush.verticalGradient(0f to tint, 0.75f to base) else Brush.verticalGradient(listOf(base, base))
    Surface(Modifier.fillMaxSize(), color = base) {
        Column(
            Modifier.fillMaxSize().background(brush).statusBarsPadding().navigationBarsPadding().padding(horizontal = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onClose) { Icon(Icons.Filled.KeyboardArrowDown, contentDescription = tr("Close player")) }
                Text(tr("Now playing"), Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
                IconButton(onClick = { lyricsOn = !lyricsOn }) {
                    Icon(
                        Icons.Filled.Lyrics,
                        contentDescription = if (lyricsOn) tr("Hide lyrics") else tr("Show lyrics"),
                        tint = if (lyricsOn) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                IconButton(onClick = { vm.library.toggleFav(t.id) }) {
                    Icon(
                        if (t.fav) Icons.Filled.Favorite else Icons.Filled.FavoriteBorder,
                        contentDescription = if (t.fav) tr("Unlike") else tr("Like"),
                        tint = if (t.fav) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            Box(Modifier.weight(1f).fillMaxWidth().padding(vertical = 8.dp), contentAlignment = Alignment.Center) {
                if (lyricsOn) LyricsPanel(vm, t)
                else BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Art(t.title, minOf(maxWidth, maxHeight, 320.dp), cover = t.cover)
                }
            }

            Text(t.title, style = MaterialTheme.typography.headlineSmall, maxLines = 2, overflow = TextOverflow.Ellipsis, textAlign = TextAlign.Center)
            Text(t.artist, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            StarRow(t.rating, size = 24.dp) { vm.rate(t.id, it) }

            var drag by remember { mutableStateOf<Float?>(null) }
            Slider(
                value = drag ?: vm.positionMs.toFloat(),
                onValueChange = { drag = it },
                onValueChangeFinished = { drag?.let { vm.seekTo(it.toLong()) }; drag = null },
                valueRange = 0f..maxOf(vm.durationMs, 1L).toFloat(),
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(fmt((drag ?: vm.positionMs.toFloat()).toLong()), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Text(fmt(vm.durationMs), style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Spacer(Modifier.height(8.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly, verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = vm::toggleShuffle) {
                    Icon(Icons.Filled.Shuffle, contentDescription = tr("Shuffle"), tint = if (vm.shuffle) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
                }
                IconButton(onClick = vm::prev) { Icon(Icons.Filled.SkipPrevious, contentDescription = tr("Previous"), modifier = Modifier.size(36.dp)) }
                FilledIconButton(onClick = vm::toggle, modifier = Modifier.size(72.dp), colors = playColors()) {
                    Icon(
                        if (vm.isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow,
                        contentDescription = if (vm.isPlaying) tr("Pause") else tr("Play"),
                        modifier = Modifier.size(40.dp),
                    )
                }
                IconButton(onClick = vm::next) { Icon(Icons.Filled.SkipNext, contentDescription = tr("Next"), modifier = Modifier.size(36.dp)) }
                IconButton(onClick = vm::toggleRepeat) {
                    Icon(Icons.Filled.Repeat, contentDescription = tr("Repeat"), tint = if (vm.repeatAll) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Spacer(Modifier.height(12.dp))
        }
    }
}

@Composable
private fun LyricsPanel(vm: MainViewModel, t: Track) {
    val ly = vm.lyrics
    when {
        vm.lyricsFor != t.id || vm.lyricsLoading -> CircularProgressIndicator()
        ly == null -> Text(tr("No lyrics found for this song"), color = MaterialTheme.colorScheme.onSurfaceVariant, textAlign = TextAlign.Center)
        ly.synced.isNotEmpty() -> {
            val lines = ly.synced
            val idx = lines.indexOfLast { it.ms <= vm.positionMs + 300 }.coerceAtLeast(0)
            val state = rememberLazyListState()
            LaunchedEffect(idx) { state.animateScrollToItem((idx - 2).coerceAtLeast(0)) }
            LazyColumn(state = state, modifier = Modifier.fillMaxSize(), horizontalAlignment = Alignment.CenterHorizontally) {
                itemsIndexed(lines) { i, l ->
                    Text(
                        l.text.ifBlank { "♪" },
                        color = if (i == idx) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                        style = if (i == idx) MaterialTheme.typography.titleLarge else MaterialTheme.typography.titleMedium,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().clickable { vm.seekTo(l.ms) }.padding(vertical = 8.dp),
                    )
                }
            }
        }
        else -> Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
            Text(ly.plain, style = MaterialTheme.typography.bodyLarge, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth())
        }
    }
}
