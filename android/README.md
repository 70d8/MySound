# MySound Android

แอปเล่นเพลงเนทีฟ (Kotlin + Jetpack Compose + Media3) ภาคมือถือของ MySound

## ความสามารถ
- เล่นเพลงเบื้องหลัง มีปุ่มควบคุมในแจ้งเตือน มินิเพลเยอร์ → หน้าเล่นเพลงเต็มจอ
- เพิ่มเพลงจากไฟล์ในเครื่อง หรือวางลิงก์ YouTube / SoundCloud (รวมเพลย์ลิสต์) ให้แอปโหลดเอง
- ปุ่ม Share ของ YouTube/SoundCloud → เลือก MySound ส่งลิงก์เข้ามา
- เพลย์ลิสต์ เพลงโปรด คะแนน 1–5 ดาว ค้นหา เรียงลำดับ แก้ชื่อ ลบ
- เนื้อเพลงจาก LRCLIB (ตัวอักษรเลื่อนตามเพลง แตะบรรทัดเพื่อข้ามไปที่นั่น)
- ธีม: สีเน้น รูปพื้นหลัง ภาษา English/ไทย

## ติดตั้งให้เพื่อน (อัปเดตอัตโนมัติด้วย Obtainium)
1. ติดตั้ง [Obtainium](https://github.com/ImranR98/Obtainium/releases) บนมือถือ
2. Obtainium → Add App → วางลิงก์ `https://github.com/70d8/MySound`
3. เปิด "Filter Release Titles/Tags by Regex" ตั้งเป็น `android-v` (กันไปหยิบ release ของ Windows)
4. กด Add แล้วติดตั้ง ต่อไปมีเวอร์ชันใหม่ Obtainium จะแจ้งให้อัปเดต

## ออกเวอร์ชันใหม่
1. แก้ `versionCode` (+1) และ `versionName` ใน `app/build.gradle.kts`
2. `git tag android-v1.0.1 && git push origin android-v1.0.1`
3. GitHub Actions (Build Android) เซ็นและแนบ APK ไว้ที่หน้า Releases

กุญแจเซ็น (`keystore/`) ห้ามหาย ห้าม commit: ถ้าเปลี่ยนกุญแจ เพื่อนต้องถอนแอปแล้วติดตั้งใหม่
