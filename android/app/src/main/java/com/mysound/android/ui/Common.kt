package com.mysound.android.ui

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlin.math.abs

/** สีปกจากชื่อเพลง (เหมือนฝั่ง PC) จนกว่าจะมีปกจริง */
fun artColor(title: String): Color = Color.hsv((abs(title.hashCode()) % 360).toFloat(), 0.45f, 0.38f)

@Composable
fun Art(title: String, size: Dp, modifier: Modifier = Modifier, cover: String? = null) {
    val bmp = remember(cover) {
        cover?.let { BitmapFactory.decodeFile(it, BitmapFactory.Options().apply { inSampleSize = 2 })?.asImageBitmap() }
    }
    if (bmp != null) {
        Image(bmp, contentDescription = null, contentScale = ContentScale.Crop, modifier = modifier.size(size).clip(RoundedCornerShape(size / 8)))
        return
    }
    Box(
        modifier.size(size).clip(RoundedCornerShape(size / 8)).background(artColor(title)),
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.Filled.MusicNote, contentDescription = null, tint = Color.White.copy(alpha = 0.6f), modifier = Modifier.size(size / 2))
    }
}
