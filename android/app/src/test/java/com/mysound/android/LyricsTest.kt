package com.mysound.android

import com.mysound.android.data.cleanForSearch
import com.mysound.android.data.parseLrc
import org.junit.Assert.assertEquals
import org.junit.Test

class LyricsTest {
    @Test
    fun parsesTimedLines() {
        val l = parseLrc("[ar:x]\n[00:12.34] Hello\n[01:02.5]World\n[00:05]Early")
        assertEquals(listOf(5000L, 12340L, 62500L), l.map { it.ms })
        assertEquals(listOf("Early", "Hello", "World"), l.map { it.text })
    }

    @Test
    fun repeatedStampsShareText() {
        val l = parseLrc("[00:10.00][00:20.00]La la")
        assertEquals(2, l.size)
        assertEquals("La la", l[1].text)
    }

    @Test
    fun plainTextHasNoTimedLines() {
        assertEquals(0, parseLrc("just words\nno stamps").size)
    }

    @Test
    fun cleansTitleNoise() {
        assertEquals("My Song", cleanForSearch("My Song (Official Video) [HD]"))
        assertEquals("Artist", cleanForSearch("Artist - Topic"))
    }
}
