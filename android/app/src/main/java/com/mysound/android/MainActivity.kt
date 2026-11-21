package com.mysound.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.ui.graphics.Color
import com.mysound.android.ui.Layouts
import com.mysound.android.ui.Look
import com.mysound.android.ui.MySoundApp
import com.mysound.android.ui.Themes
import com.mysound.android.ui.MySoundTheme

class MainActivity : ComponentActivity() {
    private val vm: MainViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        if (savedInstanceState == null) takeSharedLink(intent)
        setContent {
            val look = Look(Color(vm.accent), Themes[vm.themeIdx], Layouts[vm.layoutIdx], vm.tintOn)
            MySoundTheme(look) { MySoundApp(vm) }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        takeSharedLink(intent)
    }

    /** ปุ่มแชร์ของแอปอื่นส่งข้อความที่มีลิงก์มา: หยิบลิงก์แรกไปใส่หน้า Download */
    private fun takeSharedLink(i: Intent?) {
        if (i?.action != Intent.ACTION_SEND) return
        val text = i.getStringExtra(Intent.EXTRA_TEXT) ?: return
        Regex("https?://\\S+").find(text)?.value?.let { vm.incomingUrl = it }
    }
}
