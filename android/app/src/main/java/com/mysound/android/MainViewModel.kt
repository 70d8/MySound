package com.mysound.android

import android.app.Application
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.mysound.android.data.LibraryStore
import com.mysound.android.data.Track
import com.mysound.android.data.readTrack
import com.mysound.android.playback.PlaybackService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

class MainViewModel(app: Application) : AndroidViewModel(app) {
    private val ctx: Context = app.applicationContext
    private val prefs = ctx.getSharedPreferences("prefs", Context.MODE_PRIVATE)

    val library = LibraryStore(File(ctx.filesDir, "library.json"))

    // ---- ค่าที่ผู้ใช้ตั้ง ----
    var accent by mutableIntStateOf(prefs.getInt("accent", 0xFF5EEAD4.toInt()))
        private set

    fun chooseAccent(argb: Int) {
        accent = argb
        prefs.edit().putInt("accent", argb).apply()
    }

    // ---- สถานะการเล่น (อ่านจาก MediaController) ----
    var isPlaying by mutableStateOf(false); private set
    var currentId by mutableStateOf<Long?>(null); private set
    var positionMs by mutableLongStateOf(0L); private set
    var durationMs by mutableLongStateOf(0L); private set
    var shuffle by mutableStateOf(false); private set
    var repeatAll by mutableStateOf(false); private set
    var message by mutableStateOf<String?>(null)

    private var controller: MediaController? = null

    private val listener = object : Player.Listener {
        override fun onEvents(player: Player, events: Player.Events) = sync(player)
    }

    private fun sync(p: Player) {
        isPlaying = p.isPlaying
        currentId = p.currentMediaItem?.mediaId?.toLongOrNull()
        shuffle = p.shuffleModeEnabled
        repeatAll = p.repeatMode != Player.REPEAT_MODE_OFF
        positionMs = p.currentPosition
        durationMs = p.duration.coerceAtLeast(0L) // TIME_UNSET เป็นค่าลบ
    }

    init {
        val token = SessionToken(ctx, ComponentName(ctx, PlaybackService::class.java))
        val future = MediaController.Builder(ctx, token).buildAsync()
        future.addListener({
            try {
                val c = future.get()
                controller = c
                c.addListener(listener)
                sync(c)
            } catch (e: Exception) {
                message = "Cannot start the player: ${e.message}"
            }
        }, ContextCompat.getMainExecutor(ctx))

        // ตำแหน่งเพลงไม่มี event ให้ฟัง ต้องอ่านเป็นช่วงๆ
        viewModelScope.launch {
            while (true) {
                controller?.let { if (it.isPlaying) positionMs = it.currentPosition }
                delay(500)
            }
        }
    }

    private fun item(t: Track) = MediaItem.Builder()
        .setMediaId(t.id.toString())
        .setUri(Uri.parse(t.uri))
        .setMediaMetadata(
            MediaMetadata.Builder().setTitle(t.title).setArtist(t.artist).setAlbumTitle(t.album).build(),
        )
        .build()

    fun play(t: Track, list: List<Track>) {
        val c = controller ?: return
        c.setMediaItems(list.map(::item), list.indexOfFirst { it.id == t.id }.coerceAtLeast(0), 0L)
        c.prepare()
        c.play()
    }

    fun toggle() { controller?.let { if (it.isPlaying) it.pause() else it.play() } }
    fun next() { controller?.seekToNextMediaItem() }
    fun prev() { controller?.let { if (it.currentPosition > 3000) it.seekTo(0) else it.seekToPreviousMediaItem() } }
    fun seekTo(ms: Long) { controller?.seekTo(ms); positionMs = ms }
    fun toggleShuffle() { controller?.let { it.shuffleModeEnabled = !it.shuffleModeEnabled } }
    fun toggleRepeat() {
        controller?.let { it.repeatMode = if (it.repeatMode == Player.REPEAT_MODE_OFF) Player.REPEAT_MODE_ALL else Player.REPEAT_MODE_OFF }
    }

    fun remove(t: Track) {
        // ถ้ากำลังเล่นเพลงนี้อยู่ หยุดก่อน กันเล่นไฟล์ที่ไม่อยู่ในคลังแล้ว
        if (currentId == t.id) controller?.run { stop(); clearMediaItems() }
        library.remove(t.id)
    }

    fun import(uris: List<Uri>) {
        viewModelScope.launch {
            val known = library.tracks.map { it.uri }.toSet()
            val base = System.currentTimeMillis() * 1000
            val added = withContext(Dispatchers.IO) {
                uris.filter { it.toString() !in known }.mapIndexedNotNull { i, u ->
                    try {
                        ctx.contentResolver.takePersistableUriPermission(u, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    } catch (_: Exception) { /* บางแหล่งไม่ให้สิทธิ์ถาวร: ใช้ได้เฉพาะตอนนี้ */ }
                    readTrack(ctx, u, base + i)
                }
            }
            library.add(added)
            val skipped = uris.size - added.size
            message = "Added ${added.size} song(s)" + if (skipped > 0) ", skipped $skipped (duplicate or not audio)" else ""
        }
    }

    override fun onCleared() {
        controller?.removeListener(listener)
        controller?.release() // บริการเล่นเพลงยังทำงานต่อได้ถ้ากำลังเล่นอยู่
        super.onCleared()
    }
}
