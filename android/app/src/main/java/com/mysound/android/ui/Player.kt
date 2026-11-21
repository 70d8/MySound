package com.mysound.android.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.Shuffle
import androidx.compose.material.icons.filled.SkipNext
import androidx.compose.material.icons.filled.SkipPrevious
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.mysound.android.MainViewModel
import com.mysound.android.data.Track
import com.mysound.android.data.fmt

/** แถบเล่นเพลงเล็กเหนือแท็บล่าง: แตะเพื่อขยายเต็มจอ */
@Composable
fun MiniPlayer(vm: MainViewModel, t: Track, onOpen: () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.surfaceContainerHigh) {
        Column {
            LinearProgressIndicator(
                progress = { if (vm.durationMs > 0) (vm.positionMs.toFloat() / vm.durationMs).coerceIn(0f, 1f) else 0f },
                modifier = Modifier.fillMaxWidth().height(2.dp),
            )
            Row(
                Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(horizontal = 12.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Art(t.title, 44.dp)
                Column(Modifier.weight(1f).padding(horizontal = 12.dp)) {
                    Text(t.title, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodyLarge)
                    Text(t.artist, maxLines = 1, overflow = TextOverflow.Ellipsis, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                IconButton(onClick = vm::prev) { Icon(Icons.Filled.SkipPrevious, contentDescription = "Previous") }
                IconButton(onClick = vm::toggle) {
                    Icon(if (vm.isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow, contentDescription = if (vm.isPlaying) "Pause" else "Play")
                }
                IconButton(onClick = vm::next) { Icon(Icons.Filled.SkipNext, contentDescription = "Next") }
            }
        }
    }
}

/** หน้าเล่นเพลงเต็มจอ */
@Composable
fun NowPlaying(vm: MainViewModel, t: Track, onClose: () -> Unit) {
    Surface(Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        Column(
            Modifier.fillMaxSize().statusBarsPadding().navigationBarsPadding().padding(horizontal = 24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onClose) { Icon(Icons.Filled.KeyboardArrowDown, contentDescription = "Close player") }
                Text("Now playing", Modifier.weight(1f), style = MaterialTheme.typography.titleMedium)
                IconButton(onClick = { vm.library.toggleFav(t.id) }) {
                    Icon(
                        if (t.fav) Icons.Filled.Favorite else Icons.Filled.FavoriteBorder,
                        contentDescription = if (t.fav) "Unlike" else "Like",
                        tint = if (t.fav) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            Spacer(Modifier.weight(1f))
            Art(t.title, 280.dp)
            Spacer(Modifier.height(32.dp))
            Text(t.title, style = MaterialTheme.typography.headlineSmall, maxLines = 2, overflow = TextOverflow.Ellipsis)
            Text(t.artist, style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Spacer(Modifier.height(16.dp))

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
            Spacer(Modifier.height(16.dp))
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly, verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = vm::toggleShuffle) {
                    Icon(Icons.Filled.Shuffle, contentDescription = "Shuffle", tint = if (vm.shuffle) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
                }
                IconButton(onClick = vm::prev) { Icon(Icons.Filled.SkipPrevious, contentDescription = "Previous", modifier = Modifier.size(36.dp)) }
                FilledIconButton(onClick = vm::toggle, modifier = Modifier.size(72.dp)) {
                    Icon(
                        if (vm.isPlaying) Icons.Filled.Pause else Icons.Filled.PlayArrow,
                        contentDescription = if (vm.isPlaying) "Pause" else "Play",
                        modifier = Modifier.size(40.dp),
                    )
                }
                IconButton(onClick = vm::next) { Icon(Icons.Filled.SkipNext, contentDescription = "Next", modifier = Modifier.size(36.dp)) }
                IconButton(onClick = vm::toggleRepeat) {
                    Icon(Icons.Filled.Repeat, contentDescription = "Repeat", tint = if (vm.repeatAll) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
            Spacer(Modifier.weight(1f))
        }
    }
}
