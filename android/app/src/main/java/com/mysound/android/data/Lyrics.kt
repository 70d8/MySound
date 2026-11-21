package com.mysound.android.data

import kotlinx.serialization.json.*
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import kotlin.math.abs

data class LrcLine(val ms: Long, val text: String)
data class Lyrics(val synced: List<LrcLine>, val plain: String)

private val STAMP = Regex("\\[(\\d{1,3}):(\\d{2})(?:[.:](\\d{1,3}))?]")

/** แปลงเนื้อเพลงแบบ LRC ("[01:23.45] ข้อความ") เป็นบรรทัดที่มีเวลา เรียงตามเวลา */
fun parseLrc(src: String): List<LrcLine> {
    val out = ArrayList<LrcLine>()
    for (raw in src.lineSequence()) {
        val stamps = STAMP.findAll(raw).toList()
        if (stamps.isEmpty()) continue
        val text = raw.substring(stamps.last().range.last + 1).trim()
        for (m in stamps) {
            val frac = m.groupValues[3]
            val f = when (frac.length) {
                0 -> 0L
                1 -> frac.toLong() * 100
                2 -> frac.toLong() * 10
                else -> frac.take(3).toLong()
            }
            out += LrcLine(m.groupValues[1].toLong() * 60_000 + m.groupValues[2].toLong() * 1000 + f, text)
        }
    }
    return out.sortedBy { it.ms }
}

/** ตัดคำที่ทำให้ค้นหาไม่เจอ: "(Official Video)", "[MV]", " - Topic" */
fun cleanForSearch(s: String): String =
    s.replace(Regex("\\(.*?\\)|\\[.*?]"), " ").replace(" - Topic", "").replace(Regex("\\s+"), " ").trim()

private val json = Json { ignoreUnknownKeys = true }

private fun httpGet(url: String): String? {
    val c = URL(url).openConnection() as HttpURLConnection
    return try {
        c.connectTimeout = 8000
        c.readTimeout = 8000
        c.setRequestProperty("User-Agent", "MySound/1.0 (https://github.com/70d8/MySound)")
        if (c.responseCode in 200..299) c.inputStream.bufferedReader().use { it.readText() } else null
    } catch (_: Exception) {
        null
    } finally {
        c.disconnect()
    }
}

private fun str(o: JsonObject, k: String): String? = (o[k] as? JsonPrimitive)?.contentOrNull

private fun pick(o: JsonObject): Lyrics? {
    val synced = parseLrc(str(o, "syncedLyrics") ?: "")
    val plain = str(o, "plainLyrics") ?: ""
    return if (synced.isEmpty() && plain.isBlank()) null else Lyrics(synced, plain)
}

private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")

private fun objOf(body: String?): JsonObject? =
    body?.let { runCatching { json.parseToJsonElement(it).jsonObject }.getOrNull() }

private fun search(query: String, durSec: Long): JsonObject? {
    val arr = httpGet("https://lrclib.net/api/search?$query")
        ?.let { runCatching { json.parseToJsonElement(it).jsonArray }.getOrNull() } ?: return null
    return arr.mapNotNull { it as? JsonObject }
        .filter { pick(it) != null }
        .sortedWith(
            compareBy(
                { str(it, "syncedLyrics").isNullOrBlank() }, // เอาแบบมีเวลาก่อน
                { abs(((it["duration"] as? JsonPrimitive)?.doubleOrNull ?: 0.0) - durSec) },
            ),
        )
        .firstOrNull()
}

private fun fromNetwork(t: Track): JsonObject? {
    val title = cleanForSearch(t.title)
    val artist = cleanForSearch(t.artist).takeIf { it.isNotEmpty() && it != "Unknown artist" }
    val dur = t.durationMs / 1000
    if (artist != null) {
        val exact = objOf(
            httpGet("https://lrclib.net/api/get?track_name=${enc(title)}&artist_name=${enc(artist)}" + if (dur > 0) "&duration=$dur" else ""),
        )
        if (exact != null && pick(exact) != null) return exact
        search("track_name=${enc(title)}&artist_name=${enc(artist)}", dur)?.let { return it }
    }
    return search("track_name=${enc(title)}", dur)
}

/** ดึงเนื้อเพลงจาก LRCLIB (lrclib.net) เก็บแคชในเครื่อง คืน null ถ้าไม่เจอหรือไม่มีเน็ต (เรียกบนเธรด IO) */
fun fetchLyrics(t: Track, cacheDir: File): Lyrics? {
    cacheDir.mkdirs()
    val cache = File(cacheDir, "${t.id}.json")
    val cached = if (cache.exists()) objOf(runCatching { cache.readText() }.getOrNull()) else null
    val obj = cached ?: fromNetwork(t)?.also { runCatching { cache.writeText(it.toString()) } }
    return obj?.let(::pick)
}
