package com.mysound.android.data

import android.content.Context
import com.yausername.ffmpeg.FFmpeg
import com.yausername.youtubedl_android.YoutubeDL
import com.yausername.youtubedl_android.YoutubeDLRequest
import java.io.File

const val DL_PROCESS_ID = "dl"
private const val MARK = "MYSOUND|~|"

data class Downloaded(val id: String, val title: String, val artist: String, val durationMs: Long, val path: String)

/** แปลงบรรทัดที่ yt-dlp พิมพ์หลังโหลดเสร็จแต่ละเพลง (ดู template ใน download()) */
fun parseMarker(line: String): Downloaded? {
    val i = line.indexOf(MARK)
    if (i < 0) return null
    val p = line.substring(i + MARK.length).split("|~|")
    if (p.size < 5) return null
    fun clean(s: String) = s.trim().takeIf { it.isNotEmpty() && it != "NA" }
    val path = clean(p[4]) ?: return null
    return Downloaded(
        id = p[0].trim(),
        title = clean(p[1]) ?: File(path).nameWithoutExtension,
        artist = clean(p[2]) ?: "Unknown artist",
        durationMs = ((clean(p[3])?.toDoubleOrNull() ?: 0.0) * 1000).toLong(),
        path = path,
    )
}

/** ครอบ yt-dlp ที่ฝังในแอป (youtubedl-android) */
object Ytdl {
    private var ready = false
    private var updated = false

    @Synchronized
    fun ensureInit(ctx: Context) {
        if (ready) return
        YoutubeDL.getInstance().init(ctx)
        FFmpeg.getInstance().init(ctx)
        ready = true
    }

    /** อัปเดต yt-dlp เป็นเวอร์ชันล่าสุด (YouTube เปลี่ยนบ่อย ถ้าไม่อัปเดตจะโหลดไม่ได้) */
    @Synchronized
    fun update(ctx: Context): String {
        ensureInit(ctx)
        val s = YoutubeDL.getInstance().updateYoutubeDL(ctx)
        updated = true
        return s?.name ?: "UNKNOWN"
    }

    /** อัปเดตอัตโนมัติครั้งเดียวต่อการเปิดแอป ถ้าล้มเหลวก็ข้าม (ไม่มีเน็ตก็ยังลองโหลดต่อได้) */
    fun updateOnce(ctx: Context) {
        if (updated) return
        try { update(ctx) } catch (_: Exception) { updated = true }
    }

    /**
     * โหลดเสียงจากลิงก์ (วิดีโอเดียวหรือเพลย์ลิสต์) ลง dir เป็นไฟล์ <id>.<นามสกุล> พร้อมปก <id>.jpg
     * onLine ถูกเรียกทุกบรรทัดที่ yt-dlp พิมพ์ (progress = -1 ถ้าไม่ใช่บรรทัดความคืบหน้า)
     * คืนข้อความ stdout ทั้งหมด
     */
    fun download(ctx: Context, url: String, playlist: Boolean, dir: File, onLine: (Float, String) -> Unit): String {
        ensureInit(ctx)
        dir.mkdirs()
        val r = YoutubeDLRequest(url)
        r.addOption("-f", "bestaudio[ext=m4a]/bestaudio/best")
        r.addOption("-x")
        r.addOption("--audio-format", "best") // ไม่แปลงไฟล์ใหม่ แค่แยกเสียงออกมา
        r.addOption("--write-thumbnail")
        r.addOption("--convert-thumbnails", "jpg")
        r.addOption(if (playlist) "--yes-playlist" else "--no-playlist")
        r.addOption("--ignore-errors")
        r.addOption("--no-mtime")
        r.addOption("-o", File(dir, "%(id)s.%(ext)s").absolutePath)
        r.addOption("--progress") // --print ทำให้เงียบ ต้องสั่งให้โชว์ความคืบหน้า
        r.addOption("--print", "after_move:${MARK}%(id)s|~|%(title)s|~|%(uploader)s|~|%(duration)s|~|%(filepath)s")
        val res = YoutubeDL.getInstance().execute(r, DL_PROCESS_ID) { progress, _, line -> onLine(progress, line) }
        return res.out
    }

    fun cancel() { YoutubeDL.getInstance().destroyProcessById(DL_PROCESS_ID) }
}
