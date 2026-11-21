package com.mysound.android.data

import android.content.Context
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.provider.OpenableColumns
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File

@Serializable
data class Track(
    val id: Long,
    val title: String,
    val artist: String,
    val album: String,
    val durationMs: Long,
    val uri: String,
    val addedAt: Long,
    val fav: Boolean = false,
    val plays: Int = 0,
)

/** 83000 -> "1:23" */
fun fmt(ms: Long): String {
    val s = (ms.coerceAtLeast(0) / 1000)
    return "${s / 60}:${(s % 60).toString().padStart(2, '0')}"
}

/**
 * คลังเพลงเก็บเป็นไฟล์ JSON ไฟล์เดียว (แบบเดียวกับแอป PC)
 * shortcut: เขียนทั้งไฟล์ทุกครั้งที่แก้ บนเธรดหลัก พอคลังเกินหลักพันเพลงค่อยย้ายไป Room/SQLite
 */
class LibraryStore(private val file: File) {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    var tracks by mutableStateOf(load())
        private set

    private fun load(): List<Track> = try {
        if (file.exists()) json.decodeFromString<List<Track>>(file.readText()) else emptyList()
    } catch (_: Exception) {
        emptyList() // ไฟล์พัง: เริ่มใหม่ แต่ไม่ลบไฟล์เดิมจนกว่าจะบันทึกครั้งถัดไป
    }

    private fun update(next: List<Track>) {
        tracks = next
        val tmp = File(file.parentFile, file.name + ".tmp")
        tmp.writeText(json.encodeToString(next))
        tmp.renameTo(file) // เขียนไฟล์ชั่วคราวก่อน กันไฟล์พังถ้าแอปโดนปิดกลางคัน
    }

    fun add(list: List<Track>) = update(tracks + list)
    fun remove(id: Long) = update(tracks.filterNot { it.id == id })
    fun toggleFav(id: Long) = update(tracks.map { if (it.id == id) it.copy(fav = !it.fav) else it })
}

/** อ่านชื่อเพลง ศิลปิน อัลบั้ม ความยาว จากไฟล์เสียง (ถ้าไม่มีแท็ก ใช้ชื่อไฟล์แทน) */
fun readTrack(ctx: Context, uri: Uri, id: Long): Track? {
    val r = MediaMetadataRetriever()
    return try {
        r.setDataSource(ctx, uri)
        val name = ctx.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst()) it.getString(0) else null
        }
        val fallback = (name ?: "Unknown").substringBeforeLast('.')
        fun tag(k: Int) = r.extractMetadata(k)?.trim()?.takeIf { it.isNotEmpty() }
        Track(
            id = id,
            title = tag(MediaMetadataRetriever.METADATA_KEY_TITLE) ?: fallback,
            artist = tag(MediaMetadataRetriever.METADATA_KEY_ARTIST) ?: "Unknown artist",
            album = tag(MediaMetadataRetriever.METADATA_KEY_ALBUM) ?: "Unknown album",
            durationMs = tag(MediaMetadataRetriever.METADATA_KEY_DURATION)?.toLongOrNull() ?: 0L,
            uri = uri.toString(),
            addedAt = System.currentTimeMillis(),
        )
    } catch (_: Exception) {
        null // ไฟล์ที่ไม่ใช่เสียง หรืออ่านไม่ได้
    } finally {
        r.release()
    }
}
