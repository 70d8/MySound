// คำสั่งเสริม: เปิดลิงก์, บันทึกรูป, ข้อมูล YouTube (บท/รายการในช่อง), ตัดเพลง, แปลง MP3,
// พื้นหลังวิดีโอ, Last.fm, รีโมทผ่านมือถือ (เว็บในวง Wi-Fi เดียวกัน)
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// เปิดลิงก์ https ในเบราว์เซอร์ของเครื่อง
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") || url.len() > 2000 || url.chars().any(|c| c.is_control() || c == '"') {
        return Err("ลิงก์ไม่ถูกต้อง".into());
    }
    #[cfg(windows)]
    let r = Command::new("explorer").arg(&url).spawn();
    #[cfg(not(windows))]
    let r = Command::new("xdg-open").arg(&url).spawn();
    r.map(|_| ()).map_err(|e| e.to_string())
}

/// บันทึกรูป PNG (base64) ไปที่พาธที่ผู้ใช้เลือก
#[tauri::command]
pub fn save_png(path: String, b64: String) -> Result<(), String> {
    use base64::Engine;
    if !path.to_lowercase().ends_with(".png") { return Err("ต้องเป็นไฟล์ .png".into()); }
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim_start_matches("data:image/png;base64,")).map_err(|e| e.to_string())?;
    if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) { return Err("ข้อมูลไม่ใช่รูป PNG".into()); }
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[derive(serde::Serialize, Debug, PartialEq)]
pub struct Chapter { title: String, start: f64, end: f64 }
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct Entry { url: String, title: String }
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct YtInfo { title: String, chapters: Vec<Chapter>, entries: Vec<Entry> }

pub fn parse_info(v: &serde_json::Value) -> YtInfo {
    let s = |x: &serde_json::Value| x.as_str().unwrap_or("").to_string();
    let chapters = v["chapters"].as_array().map(|a| a.iter().filter_map(|c| Some(Chapter { title: s(&c["title"]), start: c["start_time"].as_f64()?, end: c["end_time"].as_f64()? })).collect()).unwrap_or_default();
    let entries = v["entries"].as_array().map(|a| a.iter().filter_map(|e| {
        let url = e["url"].as_str().filter(|u| u.starts_with("https://")).map(str::to_string)
            .or_else(|| e["id"].as_str().filter(|id| id.len() == 11).map(|id| format!("https://www.youtube.com/watch?v={id}")))?;
        Some(Entry { url, title: s(&e["title"]) })
    }).collect()).unwrap_or_default();
    YtInfo { title: s(&v["title"]), chapters, entries }
}

/// อ่านข้อมูลลิงก์ด้วย yt-dlp -J: บทของวิดีโอ หรือรายการเพลงล่าสุดในช่อง/เพลย์ลิสต์
#[tauri::command]
pub async fn yt_info(url: String, max: Option<u32>, cookies: Option<String>) -> Result<YtInfo, String> {
    if !url.starts_with("https://") { return Err("ลิงก์ไม่ถูกต้อง".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        let out = tool("yt-dlp")
            .args(cookie_args(cookies.as_deref().unwrap_or("")))
            .args(["-J", "--flat-playlist", "--no-warnings", "--encoding", "utf-8", "--playlist-end"])
            .arg(max.unwrap_or(15).to_string())
            .arg(&url)
            .output()
            .map_err(|e| e.to_string())?;
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| {
            String::from_utf8_lossy(&out.stderr).lines().rev().find(|l| l.contains("ERROR")).unwrap_or("อ่านข้อมูลลิงก์นี้ไม่ได้").to_string()
        })?;
        Ok(parse_info(&v))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// ตัดช่วงหนึ่งของไฟล์ในคลังเป็นไฟล์ใหม่ (ไม่เข้ารหัสใหม่ จึงเร็วและคุณภาพเท่าเดิม)
#[tauri::command]
pub async fn cut_audio(app: AppHandle, path: String, start: f64, end: f64) -> Result<String, String> {
    let music = data_dir(&app)?.join("music");
    tauri::async_runtime::spawn_blocking(move || {
        let p = std::path::Path::new(&path);
        if !p.starts_with(&music) { return Err("ไฟล์นี้ไม่ได้อยู่ในคลังของแอป".into()); }
        if !(end > start + 0.5) { return Err("ช่วงที่เลือกสั้นเกินไป".into()); }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("m4a");
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let out = music.join(format!("clip-{millis}.{ext}"));
        let (ss, t) = (format!("{start:.3}"), format!("{:.3}", end - start));
        let o = std::ffi::OsStr::new;
        ffmpeg(&[o("-ss"), o(&ss), o("-i"), p.as_os_str(), o("-t"), o(&t), o("-vn"), o("-c"), o("copy"), o("-map_metadata"), o("-1"), out.as_os_str()])?;
        Ok(out.display().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// แปลงเพลงที่เลือกเป็น MP3 (ใส่ชื่อ ศิลปิน อัลบั้ม) ลงโฟลเดอร์ปลายทาง
#[tauri::command]
pub async fn convert_mp3(items: Vec<ExportItem>, dest: String) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dest = PathBuf::from(dest);
        if !dest.is_dir() { return Err("ไม่พบโฟลเดอร์ปลายทาง".into()); }
        let mut n = 0;
        for it in items {
            let base = safe_name(&format!("{} - {}", it.artist, it.title));
            let mut out = dest.join(format!("{base}.mp3"));
            let mut k = 2;
            while out.exists() { out = dest.join(format!("{base} ({k}).mp3")); k += 1; }
            let (t, a, al) = (format!("title={}", it.title), format!("artist={}", it.artist), format!("album={}", it.album));
            let o = std::ffi::OsStr::new;
            if ffmpeg(&[o("-i"), o(&it.path), o("-vn"), o("-c:a"), o("libmp3lame"), o("-q:a"), o("2"), o("-id3v2_version"), o("3"), o("-metadata"), o(&t), o("-metadata"), o(&a), o("-metadata"), o(&al), out.as_os_str()]).is_ok() { n += 1; }
        }
        Ok(n)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// พื้นหลังวิดีโอ: คัดลอก mp4/webm เข้าโฟลเดอร์แอป (ไม่เกิน 200 MB) และลบอันเก่า
#[tauri::command]
pub fn import_bg_video(app: AppHandle, src: String, old: Option<String>) -> Result<String, String> {
    let music = data_dir(&app)?.join("music");
    let p = std::path::Path::new(src.trim().trim_matches('"'));
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "mp4" && ext != "webm" { return Err("รองรับเฉพาะวิดีโอ .mp4 และ .webm".into()); }
    if std::fs::metadata(p).map_err(|e| e.to_string())?.len() > 200 * 1024 * 1024 { return Err("วิดีโอใหญ่เกิน 200 MB".into()); }
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let out = music.join(format!("bg-video-{millis}.{ext}"));
    std::fs::copy(p, &out).map_err(|e| e.to_string())?;
    if let Some(o) = old.map(PathBuf::from).filter(|o| o.starts_with(&music) && o != &out) { let _ = std::fs::remove_file(o); }
    Ok(out.display().to_string())
}

// ---------- Last.fm (ใช้ API key/secret ของผู้ใช้เอง) ----------
const LFM: &str = "https://ws.audioscrobbler.com/2.0/";

pub fn lfm_sig(params: &[(&str, String)], secret: &str) -> String {
    let mut p: Vec<_> = params.iter().filter(|(k, _)| *k != "format" && *k != "callback").collect();
    p.sort_by(|a, b| a.0.cmp(b.0));
    let mut s: String = p.iter().map(|(k, v)| format!("{k}{v}")).collect();
    s.push_str(secret);
    format!("{:x}", md5::compute(s.as_bytes()))
}

fn lfm_call(mut params: Vec<(&str, String)>, secret: &str, post: bool) -> Result<serde_json::Value, String> {
    let sig = lfm_sig(&params, secret);
    params.push(("api_sig", sig));
    params.push(("format", "json".into()));
    let mut r = if post {
        ureq::post(LFM).header("User-Agent", "MySound").send_form(params.iter().map(|(k, v)| (*k, v.as_str())))
    } else {
        let mut q = ureq::get(LFM).header("User-Agent", "MySound");
        for (k, v) in &params { q = q.query(*k, v); }
        q.call()
    }
    .map_err(|e| format!("Last.fm: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&r.body_mut().read_to_vec().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if let Some(m) = v["message"].as_str().filter(|_| v["error"].is_number()) { return Err(format!("Last.fm: {m}")); }
    Ok(v)
}

/// ขั้นที่ 1: ขอ token แล้วให้ผู้ใช้กดอนุญาตในเบราว์เซอร์
#[tauri::command]
pub async fn lastfm_token(key: String, secret: String) -> Result<(String, String), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let v = lfm_call(vec![("method", "auth.getToken".into()), ("api_key", key.clone())], &secret, false)?;
        let t = v["token"].as_str().ok_or("Last.fm ไม่ส่ง token มา")?.to_string();
        Ok((t.clone(), format!("https://www.last.fm/api/auth/?api_key={key}&token={t}")))
    }).await.map_err(|e| e.to_string())?
}

/// ขั้นที่ 2: หลังผู้ใช้กดอนุญาตแล้ว แลก token เป็น session key
#[tauri::command]
pub async fn lastfm_session(key: String, secret: String, token: String) -> Result<(String, String), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let v = lfm_call(vec![("method", "auth.getSession".into()), ("api_key", key), ("token", token)], &secret, false)?;
        Ok((v["session"]["key"].as_str().ok_or("ยังไม่ได้กดอนุญาตในเบราว์เซอร์")?.to_string(), v["session"]["name"].as_str().unwrap_or("").to_string()))
    }).await.map_err(|e| e.to_string())?
}

/// now = true: แจ้งว่ากำลังฟัง, false: บันทึกว่าฟังจบแล้ว (scrobble)
#[tauri::command]
pub async fn lastfm_track(key: String, secret: String, sk: String, artist: String, track: String, album: String, ts: i64, dur: u32, now: bool) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut p = vec![("method", if now { "track.updateNowPlaying" } else { "track.scrobble" }.to_string()), ("api_key", key), ("sk", sk), ("artist", artist), ("track", track), ("duration", dur.to_string())];
        if !album.is_empty() { p.push(("album", album)); }
        if !now { p.push(("timestamp", ts.to_string())); }
        lfm_call(p.iter().map(|(k, v)| (*k, v.clone())).collect::<Vec<(&str, String)>>(), &secret, true).map(|_| ())
    }).await.map_err(|e| e.to_string())?
}

// ---------- รีโมทผ่านมือถือ ----------
#[derive(Default)]
pub struct Remote {
    run: Arc<AtomicBool>,
    state: Arc<Mutex<String>>,
}

#[derive(serde::Serialize)]
pub struct RemoteInfo { urls: Vec<String>, pin: String }

fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?; // ไม่ได้ส่งข้อมูลจริง แค่ให้ระบบเลือกการ์ดเครือข่าย
    Some(s.local_addr().ok()?.ip().to_string())
}

const REMOTE_HTML: &str = include_str!("remote.html");

fn query(url: &str) -> std::collections::HashMap<String, String> {
    url::form_urlencoded::parse(url.split_once('?').map(|x| x.1).unwrap_or("").as_bytes()).into_owned().collect()
}

#[tauri::command]
pub fn remote_start(app: AppHandle, state: tauri::State<'_, Remote>, port: u16) -> Result<RemoteInfo, String> {
    if state.run.swap(true, Ordering::SeqCst) { return Err("รีโมทเปิดอยู่แล้ว".into()); }
    let srv = match tiny_http::Server::http(("0.0.0.0", port)) {
        Ok(s) => s,
        Err(e) => { state.run.store(false, Ordering::SeqCst); return Err(format!("เปิดพอร์ต {port} ไม่ได้: {e}")); }
    };
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let pin = format!("{:06}", (nanos ^ 0x5bd1_e995) % 1_000_000);
    let (run, st, pin2) = (state.run.clone(), state.state.clone(), pin.clone());
    std::thread::spawn(move || {
        while run.load(Ordering::SeqCst) {
            let Ok(Some(req)) = srv.recv_timeout(std::time::Duration::from_millis(400)) else { continue };
            let u = req.url().to_string();
            let q = query(&u);
            let json = |s: String| tiny_http::Response::from_string(s).with_header(tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap());
            if q.get("k") != Some(&pin2) {
                let _ = req.respond(tiny_http::Response::from_string("PIN ไม่ถูกต้อง / wrong PIN").with_status_code(403));
                continue;
            }
            let path = u.split('?').next().unwrap_or("/");
            let _ = match path {
                "/" => req.respond(tiny_http::Response::from_string(REMOTE_HTML).with_header(tiny_http::Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())),
                "/api/state" => req.respond(json(st.lock().map(|s| s.clone()).unwrap_or_default())),
                "/api/cmd" => {
                    let c = q.get("c").cloned().unwrap_or_default();
                    if ["toggle", "next", "prev", "vol", "seek", "play"].contains(&c.as_str()) {
                        let v: Option<f64> = q.get("v").and_then(|v| v.parse().ok());
                        let _ = app.emit("np-cmd", serde_json::json!({ "cmd": c, "v": v }));
                    }
                    req.respond(json("{}".into()))
                }
                _ => req.respond(tiny_http::Response::from_string("not found").with_status_code(404)),
            };
        }
    });
    let ip = lan_ip().unwrap_or_else(|| "127.0.0.1".into());
    Ok(RemoteInfo { urls: vec![format!("http://{ip}:{port}/?k={pin}")], pin })
}

#[tauri::command]
pub fn remote_stop(state: tauri::State<'_, Remote>) {
    state.run.store(false, Ordering::SeqCst);
}

/// หน้าต่างหลักส่งสถานะ (เพลงที่เล่น + รายชื่อเพลง) มาให้รีโมทอ่าน
#[tauri::command]
pub fn remote_push(state: tauri::State<'_, Remote>, json: String) {
    if let Ok(mut s) = state.state.lock() { *s = json; }
}

#[cfg(test)]
mod tests {
    #[test]
    fn info_parse() {
        let v = serde_json::json!({"title":"Mix","chapters":[{"title":"A","start_time":0.0,"end_time":60.0},{"title":"B","start_time":60.0,"end_time":125.5}],
            "entries":[{"url":"https://www.youtube.com/watch?v=aaaaaaaaaaa","title":"x"},{"id":"bbbbbbbbbbb","title":"y"},{"id":"short","title":"bad"}]});
        let i = super::parse_info(&v);
        assert_eq!(i.chapters.len(), 2);
        assert_eq!(i.chapters[1].end, 125.5);
        assert_eq!(i.entries.len(), 2);
        assert_eq!(i.entries[1].url, "https://www.youtube.com/watch?v=bbbbbbbbbbb");
    }
    #[test]
    fn lastfm_signature() {
        // ตัวอย่างจากเอกสาร Last.fm: md5("api_keyxxxxmethodauth.getSessiontokenyyyy" + secret)
        let s = super::lfm_sig(&[("method", "auth.getSession".into()), ("api_key", "xxxx".into()), ("token", "yyyy".into()), ("format", "json".into())], "ilovecher");
        assert_eq!(s, format!("{:x}", md5::compute("api_keyxxxxmethodauth.getSessiontokenyyyyilovecher")));
    }
}

// ---------- AI จับเวลาเนื้อเพลง: whisper.cpp (โหลดตัวโปรแกรม + โมเดล small ตอนใช้ครั้งแรก) ----------
// รุ่นใหม่ล่าสุดบางรุ่นไม่มีไฟล์ Windows ให้โหลด: ล็อกรุ่นที่มีแน่ๆ ไว้ก่อน แล้วค่อยลองรุ่นอื่น
const WHISPER_ZIPS: [&str; 3] = [
    "https://github.com/ggml-org/whisper.cpp/releases/download/v1.9.2/whisper-bin-x64.zip",
    "https://github.com/ggml-org/whisper.cpp/releases/latest/download/whisper-bin-x64.zip",
    "https://github.com/ggml-org/whisper.cpp/releases/download/v1.8.3/whisper-bin-x64.zip",
];
const WHISPER_MODEL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin";
const MODEL_MIN_BYTES: u64 = 400_000_000; // ggml-small.bin ~466 MB: เล็กกว่านี้ = โหลดไม่ครบ

#[derive(Default)]
pub struct Whisper(Arc<Mutex<Option<std::process::Child>>>);

fn wdir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = data_dir(app)?.join("whisper");
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

fn find_file(dir: &std::path::Path, name: &str, depth: u8) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth > 0 { if let Some(f) = find_file(&p, name, depth - 1) { return Some(f); } }
        } else if p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)) {
            return Some(p);
        }
    }
    None
}

fn whisper_exe(app: &AppHandle) -> Option<PathBuf> {
    let name = if cfg!(windows) { "whisper-cli.exe" } else { "whisper-cli" };
    wdir(app).ok().and_then(|d| find_file(&d, name, 3))
}

#[derive(serde::Serialize)]
pub struct WhisperStatus { engine: bool, model: bool }

#[tauri::command]
pub fn whisper_status(app: AppHandle) -> Result<WhisperStatus, String> {
    let m = wdir(&app)?.join("ggml-small.bin");
    Ok(WhisperStatus { engine: whisper_exe(&app).is_some(), model: std::fs::metadata(m).map(|x| x.len() >= MODEL_MIN_BYTES).unwrap_or(false) })
}

/// โหลดไฟล์ใหญ่ลง .part แล้วค่อยเปลี่ยนชื่อ พร้อมส่งความคืบหน้าเป็น event
fn fetch_to(app: &AppHandle, url: &str, dest: &std::path::Path, stage: &str) -> Result<(), String> {
    use std::io::{Read, Write};
    let mut r = ureq::get(url).header("User-Agent", "MySound").call().map_err(|e| format!("โหลดไม่ได้: {e}"))?;
    let total: u64 = r.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
    let part = dest.with_extension("part");
    let mut f = std::fs::File::create(&part).map_err(|e| e.to_string())?;
    let mut rd = r.body_mut().with_config().limit(2_000_000_000).reader();
    let mut buf = vec![0u8; 1 << 16];
    let (mut done, mut last) = (0u64, 0u64);
    loop {
        let n = rd.read(&mut buf).map_err(|e| format!("โหลดขาดกลางทาง: {e}"))?;
        if n == 0 { break; }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        if done - last > 2_000_000 { last = done; let _ = app.emit("whisper-setup", serde_json::json!({ "stage": stage, "done": done, "total": total })); }
    }
    drop(f);
    if total > 0 && done < total { return Err("โหลดไม่ครบ ลองใหม่อีกครั้ง".into()); }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())
}

/// เตรียมตัวโปรแกรมและโมเดลให้พร้อม (โหลดเฉพาะที่ยังไม่มี)
#[tauri::command]
pub async fn whisper_setup(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let d = wdir(&app)?;
        if whisper_exe(&app).is_none() {
            if !cfg!(windows) { return Err("ตอนนี้รองรับเฉพาะ Windows".into()); }
            let zip = d.join("whisper-bin-x64.zip");
            let mut last = String::new();
            let ok = WHISPER_ZIPS.iter().any(|u| match fetch_to(&app, u, &zip, "engine") {
                Ok(()) => true,
                Err(e) => { last = e; false }
            });
            if !ok { return Err(format!("โหลดตัวโปรแกรม whisper ไม่ได้ ({last})")); }
            // Windows 10 ขึ้นไปมี tar.exe ที่แตกไฟล์ zip ได้
            let out = tool("tar").arg("-xf").arg(&zip).arg("-C").arg(&d).output().map_err(|e| format!("แตกไฟล์ไม่ได้: {e}"))?;
            let _ = std::fs::remove_file(&zip);
            if !out.status.success() || whisper_exe(&app).is_none() { return Err("แตกไฟล์ whisper ไม่สำเร็จ".into()); }
        }
        let m = d.join("ggml-small.bin");
        if std::fs::metadata(&m).map(|x| x.len() < MODEL_MIN_BYTES).unwrap_or(true) {
            fetch_to(&app, WHISPER_MODEL, &m, "model")?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize, Debug, PartialEq)]
pub struct WWord { ms: i64, text: String }

pub fn parse_whisper_json(v: &serde_json::Value) -> Vec<WWord> {
    v["transcription"].as_array().map(|a| a.iter().filter_map(|s| {
        let text = s["text"].as_str()?.to_string();
        if text.trim().is_empty() || text.trim_start().starts_with('[') { return None; } // [Music] ฯลฯ
        Some(WWord { ms: s["offsets"]["from"].as_i64()?, text })
    }).collect()).unwrap_or_default()
}

/// ให้ whisper ฟังเพลงแล้วคืนคำ/โทเค็นพร้อมเวลา (มิลลิวินาที)
#[tauri::command]
pub async fn whisper_run(app: AppHandle, state: tauri::State<'_, Whisper>, path: String, lang: String) -> Result<Vec<WWord>, String> {
    let cell = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let exe = whisper_exe(&app).ok_or("ยังไม่ได้ติดตั้ง whisper")?;
        let d = wdir(&app)?;
        let model = d.join("ggml-small.bin");
        let lang = if lang.chars().all(|c| c.is_ascii_alphabetic()) && (2..=5).contains(&lang.len()) { lang } else { "auto".into() };
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |x| x.as_millis());
        let wav = std::env::temp_dir().join(format!("mysound-w-{millis}.wav"));
        let base = std::env::temp_dir().join(format!("mysound-w-{millis}"));
        let o = std::ffi::OsStr::new;
        ffmpeg(&[o("-i"), o(&path), o("-vn"), o("-ar"), o("16000"), o("-ac"), o("1"), o("-c:a"), o("pcm_s16le"), wav.as_os_str()])?;
        let threads = std::thread::available_parallelism().map(|n| n.get().saturating_sub(1).max(1)).unwrap_or(4).to_string();
        let mut cmd = Command::new(&exe);
        cmd.arg("-m").arg(&model).arg("-f").arg(&wav).args(["-l", &lang, "-t", &threads, "-ml", "1", "-oj", "-pp", "-of"]).arg(&base)
            .stdout(Stdio::null()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn().map_err(|e| format!("เปิด whisper ไม่ได้: {e}"))?;
        let err = child.stderr.take();
        *cell.lock().map_err(|e| e.to_string())? = Some(child);
        let mut tail = String::new();
        if let Some(err) = err {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                if let Some(p) = line.split("progress =").nth(1).and_then(|x| x.trim().trim_end_matches('%').trim().parse::<u32>().ok()) {
                    let _ = app.emit("whisper-progress", p);
                } else if !line.trim().is_empty() { tail = line; }
            }
        }
        let status = cell.lock().map_err(|e| e.to_string())?.take().map(|mut c| c.wait());
        let _ = std::fs::remove_file(&wav);
        let json = base.with_extension("json");
        let ok = matches!(status, Some(Ok(s)) if s.success());
        let text = std::fs::read_to_string(&json);
        let _ = std::fs::remove_file(&json);
        if !ok { return Err(if status.is_none() { "ยกเลิกแล้ว".into() } else { format!("whisper ทำงานไม่สำเร็จ: {tail}") }); }
        let v: serde_json::Value = serde_json::from_str(&text.map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        Ok(parse_whisper_json(&v))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn whisper_cancel(state: tauri::State<'_, Whisper>) {
    if let Ok(mut g) = state.0.lock() {
        if let Some(c) = g.as_mut() { let _ = c.kill(); }
    }
}

#[cfg(test)]
mod wtests {
    #[test]
    fn whisper_json() {
        let v = serde_json::json!({"transcription":[
            {"offsets":{"from":0,"to":500},"text":" [Music]"},
            {"offsets":{"from":1200,"to":1500},"text":" Hello"},
            {"offsets":{"from":1500,"to":1800},"text":" world"},
            {"offsets":{"from":2000,"to":2100},"text":" "}]});
        let w = super::parse_whisper_json(&v);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].ms, 1200);
        assert_eq!(w[1].text, " world");
    }
}
