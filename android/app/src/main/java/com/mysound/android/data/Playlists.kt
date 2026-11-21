package com.mysound.android.data

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import java.io.File

@Serializable
data class Playlist(val id: Long, val name: String, val trackIds: List<Long> = emptyList())

/** เพลย์ลิสต์ของผู้ใช้ เก็บเป็น JSON ไฟล์เดียว (เพลงโปรดเป็นเพลย์ลิสต์อัตโนมัติ ไม่ต้องเก็บที่นี่) */
class PlaylistStore(private val file: File) {
    private val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }

    var items by mutableStateOf(load())
        private set

    private fun load(): List<Playlist> = try {
        if (file.exists()) json.decodeFromString<List<Playlist>>(file.readText()) else emptyList()
    } catch (_: Exception) {
        emptyList()
    }

    private fun update(next: List<Playlist>) {
        items = next
        val tmp = File(file.parentFile, file.name + ".tmp")
        tmp.writeText(json.encodeToString(next))
        tmp.renameTo(file)
    }

    fun create(name: String): Long {
        val id = System.currentTimeMillis()
        update(items + Playlist(id, name.trim()))
        return id
    }

    fun rename(id: Long, name: String) = update(items.map { if (it.id == id) it.copy(name = name.trim()) else it })
    fun delete(id: Long) = update(items.filterNot { it.id == id })

    fun addTrack(pid: Long, tid: Long) =
        update(items.map { if (it.id == pid && tid !in it.trackIds) it.copy(trackIds = it.trackIds + tid) else it })

    fun removeTrack(pid: Long, tid: Long) =
        update(items.map { if (it.id == pid) it.copy(trackIds = it.trackIds - tid) else it })

    /** เพลงถูกลบจากคลัง: เอาออกจากทุกเพลย์ลิสต์ */
    fun purge(tid: Long) = update(items.map { it.copy(trackIds = it.trackIds - tid) })
}
