package com.mysound.android

import android.app.Application
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.toArgb
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.mysound.android.data.Downloaded
import com.mysound.android.data.LibraryStore
import com.mysound.android.data.Lyrics
import com.mysound.android.data.PlaylistStore
import com.mysound.android.data.Track
import com.mysound.android.data.Ytdl
import com.mysound.android.data.fetchLyrics
import com.mysound.android.data.parseMarker
import com.mysound.android.data.readTrack
import com.mysound.android.playback.PlaybackService
import com.mysound.android.ui.Layouts
import com.mysound.android.ui.Themes
import com.mysound.android.ui.appLang
import com.mysound.android.ui.tr
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.atomic.AtomicLong

class MainViewModel(app: Application) : AndroidViewModel(app) {
    private val ctx: Context = app.applicationContext
    private val prefs = ctx.getSharedPreferences("prefs", Context.MODE_PRIVATE)

    val library = LibraryStore(File(ctx.filesDir, "library.json"))
    val playlists = PlaylistStore(File(ctx.filesDir, "playlists.json"))

    init { appLang = prefs.getString("lang", "en") ?: "en" }

    // ---- ค่าที่ผู้ใช้ตั้ง ----
    var themeIdx by mutableIntStateOf(prefs.getInt("theme", 0).coerceIn(0, Themes.lastIndex)); private set
    var layoutIdx by mutableIntStateOf(prefs.getInt("layout", 0).coerceIn(0, Layouts.lastIndex)); private set
    var tintOn by mutableStateOf(prefs.getBoolean("tint", true)); private set
    var accent by mutableIntStateOf(prefs.getInt("accent", Themes[themeIdx].accent.toArgb()))
        private set

    /** เลือกธีม: ใช้สีเน้นของธีมนั้น (เลือกสีเน้นเองทีหลังได้) */
    fun chooseTheme(i: Int) {
        themeIdx = i
        accent = Themes[i].accent.toArgb()
        prefs.edit().putInt("theme", i).putInt("accent", accent).apply()
    }

    fun chooseLayout(i: Int) {
        layoutIdx = i
        prefs.edit().putInt("layout", i).apply()
    }

    fun chooseTint(on: Boolean) {
        tintOn = on
        prefs.edit().putBoolean("tint", on).apply()
    }

    fun chooseAccent(argb: Int) {
        accent = argb
        prefs.edit().putInt("accent", argb).apply()
    }

    fun chooseLang(code: String) {
        appLang = code
        prefs.edit().putString("lang", code).apply()
    }

    // ---- รูปพื้นหลัง ----
    val bgFile = File(ctx.filesDir, "background.img")
    var bgVersion by mutableIntStateOf(if (bgFile.exists()) 1 else 0); private set
    var bgAlpha by mutableFloatStateOf(prefs.getFloat("bgAlpha", 0.6f)); private set

    fun chooseBgAlpha(v: Float) {
        bgAlpha = v
        prefs.edit().putFloat("bgAlpha", v).apply()
    }

    fun setBackground(uri: Uri) {
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) {
                    val input = ctx.contentResolver.openInputStream(uri) ?: error("cannot open")
                    input.use { i -> bgFile.outputStream().use { o -> i.copyTo(o) } }
                }
                bgVersion++
            } catch (_: Exception) {
                message = tr("Could not load that image")
            }
        }
    }

    fun clearBackground() {
        bgFile.delete()
        bgVersion++
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
        val id = p.currentMediaItem?.mediaId?.toLongOrNull()
        // นับจำนวนครั้งที่เล่น เมื่อเปลี่ยนมาเพลงใหม่ตั้งแต่ต้นเพลง
        if (id != null && id != currentId && p.currentPosition < 3000) library.edit(id) { it.copy(plays = it.plays + 1, lastPlayed = System.currentTimeMillis()) }
        isPlaying = p.isPlaying
        currentId = id
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

    // ---- จัดการเพลงในคลัง ----
    fun remove(t: Track) {
        // ถ้ากำลังเล่นเพลงนี้อยู่ หยุดก่อน กันเล่นไฟล์ที่ไม่อยู่ในคลังแล้ว
        if (currentId == t.id) controller?.run { stop(); clearMediaItems() }
        library.remove(t.id)
        playlists.purge(t.id)
        // เพลงที่แอปโหลดมาเองเก็บในโฟลเดอร์ของแอป ลบไฟล์ด้วย (ไฟล์จากเครื่องผู้ใช้ไม่แตะ)
        if (t.uri.startsWith("file:")) viewModelScope.launch(Dispatchers.IO) {
            Uri.parse(t.uri).path?.let { File(it).delete() }
            t.cover?.let { File(it).delete() }
        }
        viewModelScope.launch(Dispatchers.IO) { File(ctx.filesDir, "lyrics/${t.id}.json").delete() }
    }

    fun rate(id: Long, stars: Int) = library.edit(id) { it.copy(rating = if (it.rating == stars) 0 else stars) }
    fun rename(id: Long, title: String, artist: String) = library.edit(id) { it.copy(title = title, artist = artist) }

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
            message = tr("Added {0} song(s)", added.size) + if (skipped > 0) tr(", skipped {0} (duplicate or not audio)", skipped) else ""
        }
    }

    // ---- เนื้อเพลง (LRCLIB) ----
    var lyrics by mutableStateOf<Lyrics?>(null); private set
    var lyricsLoading by mutableStateOf(false); private set
    var lyricsFor by mutableStateOf<Long?>(null); private set
    private var lyricsJob: Job? = null

    fun loadLyrics(t: Track) {
        if (lyricsFor == t.id && (lyrics != null || lyricsLoading)) return
        lyricsJob?.cancel()
        lyricsFor = t.id; lyrics = null; lyricsLoading = true
        lyricsJob = viewModelScope.launch {
            val r = withContext(Dispatchers.IO) { fetchLyrics(t, File(ctx.filesDir, "lyrics")) }
            if (lyricsFor == t.id) { lyrics = r; lyricsLoading = false }
        }
    }

    // ---- ดาวน์โหลดจากลิงก์ (yt-dlp) ----
    var dlBusy by mutableStateOf(false); private set
    var dlProgress by mutableFloatStateOf(-1f); private set // -1 = ยังไม่ทราบ
    var dlStatus by mutableStateOf(""); private set
    var incomingUrl by mutableStateOf<String?>(null) // ลิงก์ที่แชร์เข้ามา รอใส่ช่อง Download
    private val dlSeq = AtomicLong(System.currentTimeMillis() * 1000)
    private val musicDir = File(ctx.filesDir, "music")

    private fun addDownloaded(d: Downloaded): Boolean {
        val uri = Uri.fromFile(File(d.path)).toString()
        if (library.tracks.any { it.uri == uri } || !File(d.path).exists()) return false
        val cover = File(musicDir, d.id + ".jpg").takeIf { it.exists() }?.absolutePath
        library.add(listOf(Track(dlSeq.incrementAndGet(), d.title, d.artist, "YouTube / SoundCloud", d.durationMs, uri, System.currentTimeMillis(), cover = cover)))
        return true
    }

    fun download(url: String, playlist: Boolean) {
        val u = url.trim()
        if (dlBusy) return
        if (!u.startsWith("http://") && !u.startsWith("https://")) { message = tr("Paste a full link starting with https://"); return }
        dlBusy = true; dlProgress = -1f; dlStatus = tr("Preparing downloader…")
        viewModelScope.launch {
            var added = 0
            var canceled = false
            try {
                withContext(Dispatchers.IO) {
                    Ytdl.ensureInit(ctx)
                    withContext(Dispatchers.Main) { dlStatus = tr("Checking for downloader updates…") }
                    Ytdl.updateOnce(ctx)
                    withContext(Dispatchers.Main) { dlStatus = tr("Downloading…") }
                    val out = Ytdl.download(ctx, u, playlist, musicDir) { p, line ->
                        viewModelScope.launch(Dispatchers.Main.immediate) {
                            if (p >= 0f) dlProgress = p / 100f
                            parseMarker(line)?.let { if (addDownloaded(it)) { added++; dlProgress = -1f } }
                        }
                    }
                    // เผื่อบรรทัดไหนไม่ได้ผ่าน callback: อ่านซ้ำจากผลลัพธ์ทั้งหมด (ข้ามอันที่เพิ่มแล้ว)
                    withContext(Dispatchers.Main) { out.lineSequence().mapNotNull(::parseMarker).forEach { if (addDownloaded(it)) added++ } }
                }
            } catch (e: Exception) {
                canceled = e.javaClass.simpleName.contains("Cancel")
                if (!canceled) {
                    val why = e.message?.lineSequence()?.lastOrNull { it.contains("ERROR") } ?: e.message ?: e.javaClass.simpleName
                    message = tr("Download failed: {0}", why.take(240))
                }
            } finally {
                dlBusy = false; dlProgress = -1f; dlStatus = ""
            }
            if (canceled) message = tr("Download canceled")
            else if (added > 0) message = tr("Downloaded {0} song(s)", added)
        }
    }

    fun cancelDownload() { viewModelScope.launch(Dispatchers.IO) { try { Ytdl.cancel() } catch (_: Exception) {} } }

    fun updateEngine() {
        if (dlBusy) return
        dlBusy = true; dlStatus = tr("Updating downloader…")
        viewModelScope.launch {
            message = try {
                val r = withContext(Dispatchers.IO) { Ytdl.update(ctx) }
                if (r == "ALREADY_UP_TO_DATE") tr("Downloader is already up to date") else tr("Downloader updated")
            } catch (e: Exception) { tr("Update failed: {0}", e.message?.take(200) ?: "") }
            dlBusy = false; dlStatus = ""
        }
    }

    override fun onCleared() {
        controller?.removeListener(listener)
        controller?.release() // บริการเล่นเพลงยังทำงานต่อได้ถ้ากำลังเล่นอยู่
        super.onCleared()
    }
}
