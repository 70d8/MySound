package com.mysound.android

import com.mysound.android.data.Track
import com.mysound.android.data.fmt
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

class LibraryTest {
    @Test
    fun formatsDuration() {
        assertEquals("0:00", fmt(0))
        assertEquals("1:23", fmt(83_000))
        assertEquals("10:05", fmt(605_999))
        assertEquals("0:00", fmt(-5)) // TIME_UNSET และค่าติดลบอื่นๆ
    }

    @Test
    fun oldLibraryFilesStillLoad() {
        // ไฟล์ที่ไม่มี field fav/plays (เวอร์ชันเก่า) ต้องอ่านได้ และ field ที่ไม่รู้จักถูกข้าม
        val json = Json { ignoreUnknownKeys = true; encodeDefaults = true }
        val old = """[{"id":1,"title":"A","artist":"B","album":"C","durationMs":1000,"uri":"content://x","addedAt":5,"future":1}]"""
        val t = json.decodeFromString<List<Track>>(old).single()
        assertEquals(false, t.fav)
        assertEquals(0, t.plays)
        assertEquals(t, json.decodeFromString<List<Track>>(json.encodeToString(listOf(t))).single())
    }
}
