# MySound — สิ่งที่ต้องติดตั้ง (Windows)

ทำครั้งเดียวก่อนเริ่มรันโปรเจกต์ ทำตามลำดับ แล้ว **ปิดและเปิด Terminal ใหม่** หลังติดตั้งเสร็จทุกอย่าง

## ต้องติดตั้ง 3 อย่าง

| # | โปรแกรม | ใช้ทำอะไร | ติดตั้ง |
|---|---|---|---|
| 1 | Microsoft C++ Build Tools | Rust บน Windows ต้องใช้ตัว compile | ดูคำสั่งด้านล่าง (ไฟล์ใหญ่ ~ 3–6 GB ใช้เวลานานสุด) |
| 2 | Rust | เขียน/สร้างส่วนหลังบ้านของแอป | `winget install Rustlang.Rustup` |
| 3 | Node.js (LTS) | สร้างหน้าตาแอป (Svelte) | `winget install OpenJS.NodeJS.LTS` |

### คำสั่งติดตั้ง (เปิด PowerShell แล้ววางทีละบรรทัด)

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Rustlang.Rustup
winget install OpenJS.NodeJS.LTS
```

- ถ้าไม่มี `winget` ให้ติดตั้งแบบกดเองจาก: visualstudio.microsoft.com/visual-cpp-build-tools (เลือก "Desktop development with C++"), rustup.rs, nodejs.org
- หลังติดตั้ง Rust ถ้ามีตัวเลือกให้เลือก ให้กด Enter เพื่อใช้ค่าเริ่มต้น (default)

## ตรวจว่าติดตั้งครบ

เปิด PowerShell **หน้าต่างใหม่** แล้วรัน:

```powershell
rustc --version
cargo --version
node --version
npm --version
```

ทั้ง 4 คำสั่งต้องแสดงเลขเวอร์ชัน ถ้าขึ้น "not recognized" แปลว่ายังไม่ได้ปิด-เปิด Terminal ใหม่ หรือติดตั้งไม่สำเร็จ

## มีอยู่แล้ว / ไม่ต้องลงเอง

- **WebView2** — Windows 11 มีมาให้แล้ว (Windows 10 ส่วนใหญ่ก็มี) ถ้าตอนรันแอปแล้วขึ้น error เรื่อง WebView2 ค่อยลง: `winget install Microsoft.EdgeWebView2Runtime`
- **yt-dlp** ต้องลงเอง (แอปเรียกใช้จาก PATH): `winget install yt-dlp.yt-dlp` แล้ว **ปิดแล้วเปิด PowerShell และแอปใหม่** เพื่อให้ PATH อัปเดต ทดสอบด้วย `yt-dlp --version`
- อัปเดต yt-dlp เมื่อดาวน์โหลดเริ่มพัง (YouTube เปลี่ยนบ่อย): `winget upgrade yt-dlp.yt-dlp`

## ตัวเลือกเสริม (ไม่บังคับ)

- ffmpeg (ใช้ปรับเสียงตัดเงียบ/ปรับความดัง และฝังแท็ก+ปกตอนส่งออก): `winget install Gyan.FFmpeg` แล้วปิดและเปิด PowerShell กับแอปใหม่
- Git: `winget install Git.Git` (ถ้าอยากเก็บประวัติการแก้โค้ด)
- VS Code: `winget install Microsoft.VisualStudioCode` (ไว้เปิดแก้โค้ดเอง)

## ถ้าติดปัญหา

ส่งข้อความ error ทั้งหมดมาให้ผม บอกด้วยว่าติดตอนรันคำสั่งไหน
อ้างอิงต้นทาง: v2.tauri.app/start/prerequisites
