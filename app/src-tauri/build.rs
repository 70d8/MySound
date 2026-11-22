fn main() {
    // ไอคอนเปลี่ยน = ต้องฝังเข้าโปรแกรมใหม่ (ปกติ cargo ไม่รู้ว่าไฟล์ในโฟลเดอร์ icons เปลี่ยน)
    println!("cargo:rerun-if-changed=icons");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    tauri_build::build()
}
