package com.mysound.android.ui

import android.Manifest
import android.graphics.BitmapFactory
import android.os.Build
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.Home
import androidx.compose.material.icons.filled.LibraryMusic
import androidx.compose.material.icons.filled.Palette
import androidx.compose.material.icons.filled.QueueMusic
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import com.mysound.android.MainViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

private data class NavTab(val label: String, val icon: ImageVector)

/** อ่านรูปพื้นหลังแบบย่อขนาด (ไม่เกินราว 1600px) กันกินหน่วยความจำ */
private fun decodeBackground(f: File): ImageBitmap? {
    if (!f.exists()) return null
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(f.path, bounds)
    var sample = 1
    while (maxOf(bounds.outWidth, bounds.outHeight) / sample > 1600) sample *= 2
    return BitmapFactory.decodeFile(f.path, BitmapFactory.Options().apply { inSampleSize = sample })?.asImageBitmap()
}

@Composable
fun MySoundApp(vm: MainViewModel) {
    val tabs = listOf(
        NavTab(tr("Home"), Icons.Filled.Home),
        NavTab(tr("Songs"), Icons.Filled.LibraryMusic),
        NavTab(tr("Playlists"), Icons.Filled.QueueMusic),
        NavTab(tr("Download"), Icons.Filled.Download),
        NavTab(tr("Appearance"), Icons.Filled.Palette),
    )
    var tab by rememberSaveable { mutableIntStateOf(0) }
    var full by rememberSaveable { mutableStateOf(false) }
    val current = vm.library.tracks.firstOrNull { it.id == vm.currentId }
    val snack = remember { SnackbarHostState() }

    // ขอสิทธิ์แสดงการแจ้งเตือน (Android 13+) ไม่งั้นปุ่มควบคุมเพลงในแจ้งเตือนจะไม่ขึ้น
    val notifPermission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    LaunchedEffect(Unit) {
        if (Build.VERSION.SDK_INT >= 33) notifPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isNotEmpty()) vm.import(uris)
    }
    val onAdd = { picker.launch(arrayOf("audio/*")) }

    LaunchedEffect(vm.message) {
        vm.message?.let { snack.showSnackbar(it); vm.message = null }
    }
    LaunchedEffect(vm.incomingUrl) { if (vm.incomingUrl != null) tab = 3 }

    BackHandler(full) { full = false }

    val bg by produceState<ImageBitmap?>(null, vm.bgVersion) { value = withContext(Dispatchers.IO) { decodeBackground(vm.bgFile) } }
    val base = MaterialTheme.colorScheme.background

    Box(Modifier.fillMaxSize().background(base)) {
        bg?.let {
            Image(it, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
            Box(Modifier.fillMaxSize().background(base.copy(alpha = vm.bgDim)))
        }
        Scaffold(
            containerColor = if (bg != null) Color.Transparent else base,
            snackbarHost = { SnackbarHost(snack) },
            bottomBar = {
                Column {
                    if (current != null) MiniPlayer(vm, current) { full = true }
                    NavigationBar(containerColor = MaterialTheme.colorScheme.surfaceContainer.copy(alpha = 0.94f)) {
                        tabs.forEachIndexed { i, t ->
                            NavigationBarItem(
                                selected = tab == i,
                                onClick = { tab = i },
                                icon = { Icon(t.icon, contentDescription = null) },
                                label = { Text(t.label, maxLines = 1) },
                            )
                        }
                    }
                }
            },
        ) { pad ->
            Box(Modifier.padding(pad).fillMaxSize()) {
                when (tab) {
                    0 -> HomeScreen(vm, onAdd)
                    1 -> SongsScreen(vm, onAdd)
                    2 -> PlaylistsScreen(vm, backEnabled = !full)
                    3 -> DownloadScreen(vm)
                    else -> SettingsScreen(vm)
                }
            }
        }
        AnimatedVisibility(
            visible = full && current != null,
            enter = slideInVertically { it },
            exit = slideOutVertically { it },
        ) {
            if (current != null) NowPlaying(vm, current) { full = false }
        }
    }
}
