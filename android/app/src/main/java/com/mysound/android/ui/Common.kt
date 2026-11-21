package com.mysound.android.ui

import android.graphics.Bitmap
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlin.math.abs

/** สีปกจากชื่อเพลง (เหมือนฝั่ง PC) จนกว่าจะมีปกจริง */
fun artColor(title: String): Color = Color.hsv((abs(title.hashCode()) % 360).toFloat(), 0.45f, 0.38f)

/** สีหลักของปก (เฉลี่ยทั้งรูปแล้วทำให้เข้มลง) ใช้ไล่สีหน้าเล่นเพลง ไม่มีปกก็ใช้สีจากชื่อ */
@Composable
fun rememberTint(title: String, cover: String?): Color {
    val fallback = artColor(title)
    val c by produceState(fallback, title, cover) {
        value = if (cover == null) fallback else withContext(Dispatchers.IO) {
            try {
                val b = BitmapFactory.decodeFile(cover, BitmapFactory.Options().apply { inSampleSize = 8 })
                val px = Bitmap.createScaledBitmap(b!!, 1, 1, true).getPixel(0, 0)
                Color(
                    android.graphics.Color.red(px) / 255f * 0.55f,
                    android.graphics.Color.green(px) / 255f * 0.55f,
                    android.graphics.Color.blue(px) / 255f * 0.55f,
                )
            } catch (_: Exception) {
                fallback
            }
        }
    }
    return c
}

@Composable
fun Art(title: String, size: Dp, modifier: Modifier = Modifier, cover: String? = null) {
    val shape = RoundedCornerShape(if (size > 150.dp) 8.dp else 4.dp)
    val bmp = remember(cover) {
        cover?.let { BitmapFactory.decodeFile(it, BitmapFactory.Options().apply { inSampleSize = 2 })?.asImageBitmap() }
    }
    if (bmp != null) {
        Image(bmp, contentDescription = null, contentScale = ContentScale.Crop, modifier = modifier.size(size).clip(shape))
        return
    }
    Box(modifier.size(size).clip(shape).background(artColor(title)), contentAlignment = Alignment.Center) {
        Icon(Icons.Filled.MusicNote, contentDescription = null, tint = Color.White.copy(alpha = 0.6f), modifier = Modifier.size(size / 2))
    }
}
