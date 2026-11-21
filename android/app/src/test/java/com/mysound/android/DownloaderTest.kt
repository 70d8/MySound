package com.mysound.android

import com.mysound.android.data.parseMarker
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DownloaderTest {
    @Test
    fun parsesFinishedLine() {
        val d = parseMarker("MYSOUND|~|abc123|~|My Song|~|Some Artist|~|215.5|~|/data/music/abc123.m4a")!!
        assertEquals("abc123", d.id)
        assertEquals("My Song", d.title)
        assertEquals("Some Artist", d.artist)
        assertEquals(215_500L, d.durationMs)
        assertEquals("/data/music/abc123.m4a", d.path)
    }

    @Test
    fun missingFieldsFallBack() {
        val d = parseMarker("MYSOUND|~|x|~|NA|~|NA|~|NA|~|/m/x.opus")!!
        assertEquals("x", d.title)
        assertEquals("Unknown artist", d.artist)
        assertEquals(0L, d.durationMs)
    }

    @Test
    fun ignoresOtherLines() {
        assertNull(parseMarker("[download]  42.0% of 3.00MiB"))
        assertNull(parseMarker("MYSOUND|~|x|~|t|~|a"))
    }
}
