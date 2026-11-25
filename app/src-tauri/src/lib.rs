mod ytdlp;

use discord_rich_presence::{activity::{Activity, ActivityType, Assets, Timestamps}, DiscordIpc, DiscordIpcClient};
use std::sync::Mutex;

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter, Manager};
use ytdlp::Line;
mod extra;
mod spotify;

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(d.join("music")).map_err(|e| e.to_string())?;
    Ok(d)
}

/// คืนข้อความ JSON ของคลังเพลง (ว่างถ้ายังไม่เคยบันทึก)
/// เลือกข้อความคลังที่ใช้ได้: ไฟล์หลักถ้าเป็น JSON ที่อ่านได้ ไม่งั้นลองไฟล์สำรอง (กันคลังหายถ้าไฟล์หลักพังจากไฟดับ/ดิสก์เต็ม)
fn pick_library(main: &str, bak: Option<&str>) -> String {
    let ok = |t: &str| serde_json::from_str::<serde_json::Value>(t).is_ok_and(|v| v.is_object());
    if main.trim().is_empty() || ok(main) { return main.to_string(); }
    match bak { Some(b) if ok(b) => b.to_string(), _ => main.to_string() }
}

#[tauri::command]
fn read_library(app: AppHandle) -> Result<String, String> {
    let dir = data_dir(&app)?;
    let main = match std::fs::read_to_string(dir.join("library.json")) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let bak = std::fs::read_to_string(dir.join("library.json.bak")).ok();
    Ok(pick_library(&main, bak.as_deref()))
}

/// เขียนไฟล์ชั่วคราวก่อนแล้วค่อย rename กันไฟล์พังถ้าปิดแอปกลางคัน
/// และเก็บสำเนาของเวอร์ชันก่อนหน้าเป็น library.json.bak (ไม่ถี่กว่าทุก 10 นาที) ไว้กู้
#[tauri::command]
fn write_library(app: AppHandle, json: String) -> Result<(), String> {
    let dir = data_dir(&app)?;
    let (main, bak) = (dir.join("library.json"), dir.join("library.json.bak"));
    let stale = std::fs::metadata(&bak).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map_or(true, |e| e.as_secs() > 600);
    if stale && std::fs::read_to_string(&main).is_ok_and(|t| pick_library(&t, None) == t) { let _ = std::fs::copy(&main, &bak); }
    let tmp = dir.join("library.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, main).map_err(|e| e.to_string())
}

#[derive(Clone, serde::Serialize)]
struct Progress {
    id: u64,
    pct: f32,
    title: Option<String>,
    state: &'static str,
    item: Option<ytdlp::Item>,
}

/// YouTube ขอให้ล็อกอินยืนยันว่าไม่ใช่บอท: ยืมคุกกี้จากเบราว์เซอร์ที่ผู้ใช้เลือก หรือจากไฟล์ cookies.txt ("file:พาธ")
fn cookie_args(c: &str) -> Vec<String> {
    if let Some(path) = c.strip_prefix("file:") {
        let path = path.trim().trim_matches('"');
        if !path.is_empty() { return vec!["--cookies".into(), path.into()]; }
    } else if ["firefox", "edge", "chrome", "brave", "opera", "vivaldi"].contains(&c) {
        return vec!["--cookies-from-browser".into(), c.into()];
    }
    vec![]
}

fn run_download(app: &AppHandle, id: u64, url: &str, playlist: bool, cookies: &str) -> Result<usize, String> {
    let music = data_dir(app)?.join("music");
    let mut cmd = Command::new(tool_path("yt-dlp"));
    cmd.args(cookie_args(cookies));
    cmd.args([if playlist { "--yes-playlist" } else { "--no-playlist" }, "--newline", "--progress", "--encoding", "utf-8"])
        // เสียงอย่างเดียว เลือก m4a ก่อน (เล่นได้ใน WebView2 โดยไม่ต้องใช้ ffmpeg)
        .args(["-f", "bestaudio[ext=m4a]/bestaudio[ext=mp3]/bestaudio"])
        .args(["-o", &format!("{}/%(id)s.%(ext)s", music.display())])
        .args(["--progress-template", &format!("download:{} %(progress._percent_str)s", ytdlp::P_PROGRESS)])
        .args(["--print", &format!("before_dl:{}\t%(playlist_index)s\t%(n_entries)s\t%(playlist_title)s\t%(title)s", ytdlp::P_TITLE)])
        .args([
            "--print",
            &format!("after_move:{}\t%(title)s\t%(uploader)s\t%(album)s\t%(duration)s\t%(playlist_title)s\t%(webpage_url)s\t%(thumbnail)s\t%(filepath)s", ytdlp::P_DONE),
        ])
        .args(if is_sc(url) {
            // SoundCloud มีปกให้ในตัว: เก็บเป็น <id>.cover.<ext>
            vec!["--write-thumbnail".to_string(), "-o".into(), format!("thumbnail:{}/%(id)s.cover.%(ext)s", music.display())]
        } else {
            vec![]
        })
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: ไม่เด้งหน้าต่างดำ
    }

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "ไม่พบ yt-dlp ติดตั้งด้วย winget install yt-dlp.yt-dlp แล้วปิดและเปิดแอปใหม่".to_string()
        } else {
            e.to_string()
        }
    })?;

    // อ่าน stderr ในเธรดแยก ไม่งั้นท่อเต็มแล้วโปรเซสค้าง
    let err_pipe = child.stderr.take().ok_or("no stderr")?;
    let err_thread = std::thread::spawn(move || {
        BufReader::new(err_pipe).lines().map_while(Result::ok).filter(|l| l.contains("ERROR")).map(|l| ytdlp::short_error(&l)).collect::<Vec<_>>()
    });

    let emit = |pct, item: Option<ytdlp::Item>, state| {
        let _ = app.emit("dl-progress", Progress { id, pct, title: item.as_ref().map(|i| i.title.clone()), state, item });
    };
    let mut count = 0;
    for line in BufReader::new(child.stdout.take().ok_or("no stdout")?).lines().map_while(Result::ok) {
        match ytdlp::parse(&line) {
            Some(Line::Progress(p)) => emit(p, None, "downloading"),
            Some(Line::Title(t)) => emit(0.0, Some(t), "downloading"),
            Some(Line::Done(mut d)) => {
                // d.thumb = ลิงก์รูปบนเน็ต (ไว้ส่งให้ Discord) ถ้าได้ปกจาก Genius จะใช้ลิงก์ของ Genius แทน
                d.cover = if is_sc(url) {
                    sc_thumbnail(&music, &d.path)
                } else {
                    // ค้นปกจาก Genius ไม่เจอ/ออนไลน์ไม่ได้ ไม่ถือว่าโหลดล้มเหลว
                    let artist = if d.artist == "ไม่ทราบศิลปิน" { "" } else { d.artist.as_str() };
                    let stem = std::path::Path::new(&d.path).file_stem().and_then(|s| s.to_str()).unwrap_or("x").to_string();
                    genius_cover_inner(&music, &stem, &ytdlp::clean_query(&d.title, artist)).ok().map(|(path, img)| {
                        d.thumb = Some(img);
                        path
                    })
                };
                count += 1;
                let _ = app.emit("dl-done", (id, d)); // ส่งทีละเพลง เพลย์ลิสต์จะได้ขึ้นทันทีไม่ต้องรอจบ
            }
            None => {}
        }
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let errs = err_thread.join().unwrap_or_default();
    let _ = status;
    // เพลงในเพลย์ลิสต์ที่โหลดไม่ได้ (ถูกลบ/ส่วนตัว/บล็อกประเทศ) แจ้งให้หน้าจอรู้ ไม่เงียบหาย
    if count > 0 && !errs.is_empty() {
        let _ = app.emit("dl-skip", (id, errs.clone())); // ส่งทุกสาเหตุ หน้าจอจะสรุปเป็นกลุ่มให้
    }
    if count > 0 {
        Ok(count) // ถ้ามีบางเพลงในเพลย์ลิสต์พัง แต่ได้อย่างน้อย 1 เพลงก็ถือว่าสำเร็จ
    } else {
        Err(errs.last().cloned().unwrap_or_else(|| "ดาวน์โหลดไม่สำเร็จ".into()))
    }
}

fn is_sc(url: &str) -> bool {
    url.contains("soundcloud.com")
}

fn sc_thumbnail(music: &std::path::Path, audio: &str) -> Option<String> {
    let stem = std::path::Path::new(audio).file_stem()?.to_str()?.to_string();
    let prefix = format!("{stem}.cover.");
    std::fs::read_dir(music).ok()?.flatten().find(|e| e.file_name().to_string_lossy().starts_with(&prefix)).map(|e| e.path().display().to_string())
}

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let mut r = ureq::get(url).header("User-Agent", UA).call().map_err(|e| format!("โหลดจาก {url} ไม่ได้: {e}"))?;
    r.body_mut().with_config().limit(15_000_000).read_to_vec().map_err(|e| e.to_string())
}

fn store_cover(music: &std::path::Path, key: &str, bytes: &[u8], old: Option<&str>) -> Result<String, String> {
    let ext = match bytes {
        [0x89, b'P', ..] => "png",
        [0xFF, 0xD8, ..] => "jpg",
        [b'G', b'I', b'F', b'8', ..] => "gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
        _ => return Err("ไฟล์นี้ไม่ใช่รูป (รองรับ jpg, png, webp, gif)".into()),
    };
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let path = music.join(format!("{key}.{ts}.cover.{ext}")); // ชื่อใหม่ทุกครั้ง กันภาพเก่าค้างในแคช
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    if let Some(o) = old {
        let o = std::path::Path::new(o);
        if o.starts_with(music) {
            let _ = std::fs::remove_file(o);
        }
    }
    Ok(path.display().to_string())
}

fn genius_cover_inner(music: &std::path::Path, key: &str, query: &str) -> Result<(String, String), String> {
    if query.trim().is_empty() {
        return Err("ไม่มีคำค้น".into());
    }
    let q: String = url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
    let body = http_get(&format!("https://genius.com/api/search/song?q={q}"))?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let img = ytdlp::pick_cover(&v).ok_or("Genius ไม่พบปกของเพลงนี้")?;
    let path = store_cover(music, key, &http_get(&img)?, None)?;
    Ok((path, img))
}

#[derive(serde::Serialize)]
struct CoverResult {
    path: String,
    url: Option<String>,
}

/// ค้นปกจาก Genius ด้วยชื่อ/ศิลปินที่ผู้ใช้แก้
#[tauri::command]
async fn genius_cover(app: AppHandle, key: String, query: String, old: Option<String>) -> Result<CoverResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let music = data_dir(&app)?.join("music");
        let (new, img) = genius_cover_inner(&music, &key, &query)?;
        if let Some(o) = old.as_deref().map(std::path::Path::new).filter(|o| o.starts_with(&music)) {
            let _ = std::fs::remove_file(o);
        }
        Ok(CoverResult { path: new, url: Some(img) })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// ใช้รูปปกจากลิงก์รูป หรือไฟล์ในเครื่อง (วางพาธได้ ทั้งแบบมี "" ครอบ)
#[tauri::command]
async fn import_cover(app: AppHandle, key: String, source: String, old: Option<String>) -> Result<CoverResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let music = data_dir(&app)?.join("music");
        let src = source.trim().trim_matches('"');
        let remote = src.starts_with("https://");
        let bytes = if remote || src.starts_with("http://") {
            http_get(src)?
        } else {
            std::fs::read(src).map_err(|e| format!("อ่านไฟล์ไม่ได้: {e}"))?
        };
        let path = store_cover(&music, &key, &bytes, old.as_deref())?;
        Ok(CoverResult { path, url: remote.then(|| src.to_string()) }) // รูปจากไฟล์ในเครื่อง Discord อ่านไม่ได้ จึงไม่มี url
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn download(app: AppHandle, id: u64, url: String, playlist: bool, cookies: Option<String>) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || run_download(&app, id, &url, playlist, cookies.as_deref().unwrap_or("")))
        .await
        .map_err(|e| e.to_string())?
}


// ---------- เครื่องมือ: ตรวจไฟล์ / ลบ / เวอร์ชัน / อัปเดต / ffmpeg / ส่งออก ----------

/// หาเครื่องมือในโฟลเดอร์ bin ข้างตัวโปรแกรมก่อน (ตัวติดตั้งแพ็กมาให้) ไม่เจอค่อยใช้ที่ติดตั้งในเครื่อง (PATH)
fn tool_path(name: &str) -> std::ffi::OsString {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("bin").join(format!("{name}.exe"))))
        .filter(|p| p.is_file())
        .map(Into::into)
        .unwrap_or_else(|| name.into())
}

fn tool(name: &str) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(tool_path(name));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

#[tauri::command]
fn missing_files(paths: Vec<String>) -> Vec<String> {
    paths.into_iter().filter(|p| !std::path::Path::new(p).exists()).collect()
}

/// ลบไฟล์เฉพาะที่อยู่ในโฟลเดอร์ music ของแอปเท่านั้น (กันพาธแปลกๆ)
#[tauri::command]
fn delete_files(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    let music = data_dir(&app)?.join("music");
    for p in paths {
        let p = std::path::Path::new(&p);
        if p.starts_with(&music) {
            let _ = std::fs::remove_file(p);
        }
    }
    Ok(())
}

/// เวอร์ชันของ yt-dlp / ffmpeg (None = ไม่พบ)
#[tauri::command]
fn tool_version(name: String) -> Option<String> {
    let (bin, arg) = match name.as_str() {
        "yt-dlp" => ("yt-dlp", "--version"),
        "ffmpeg" => ("ffmpeg", "-version"),
        _ => return None,
    };
    let out = tool(bin).arg(arg).output().ok()?;
    let first = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_string();
    // ffmpeg: "ffmpeg version 7.1 Copyright ..." -> "7.1"
    Some(if bin == "ffmpeg" { first.split_whitespace().nth(2).unwrap_or(&first).to_string() } else { first })
}

#[tauri::command]
async fn ytdlp_update() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let run = |c: &mut Command| c.output().map(|o| (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))));
        let tail = |s: &str| s.lines().rev().filter(|l| !l.trim().is_empty()).take(3).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        match run(tool("yt-dlp").arg("-U")) {
            Ok((true, o)) => Ok(tail(&o)),
            _ => match run(tool("winget").args(["upgrade", "yt-dlp.yt-dlp", "--silent", "--accept-source-agreements", "--accept-package-agreements"])) {
                Ok((true, o)) => Ok(tail(&o)),
                Ok((false, o)) => Err(tail(&o)),
                Err(_) => Err("อัปเดตไม่ได้ ลองรัน winget upgrade yt-dlp.yt-dlp ใน PowerShell เอง".into()),
            },
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

const NO_FFMPEG: &str = "ไม่พบ ffmpeg ติดตั้งด้วย winget install Gyan.FFmpeg แล้วปิดและเปิดแอปใหม่";

fn ffmpeg(args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let out = tool("ffmpeg").args(["-y", "-hide_banner", "-loglevel", "error"]).args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound { NO_FFMPEG.to_string() } else { e.to_string() }
    })?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("ffmpeg ล้มเหลว").to_string())
    }
}

#[derive(serde::Serialize)]
struct Processed {
    path: String,
    dur: u32,
}

/// ตัดความเงียบหัว-ท้าย + ปรับความดังให้เท่ากัน ได้ไฟล์ใหม่ (ชื่อใหม่กันแคชเก่า) แล้วลบไฟล์เดิม
#[tauri::command]
async fn process_audio(app: AppHandle, path: String) -> Result<Processed, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let music = data_dir(&app)?.join("music");
        let p = std::path::Path::new(&path);
        if !p.starts_with(&music) {
            return Err("ไฟล์นี้ไม่ได้อยู่ในคลังของแอป".into());
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let codec: [&str; 4] = match ext.as_str() {
            "m4a" | "mp4" => ["-c:a", "aac", "-b:a", "192k"],
            "mp3" => ["-c:a", "libmp3lame", "-q:a", "2"],
            "opus" | "webm" => ["-c:a", "libopus", "-b:a", "160k"],
            _ => return Err(format!("ยังไม่รองรับไฟล์ .{ext}")),
        };
        let sil = "silenceremove=start_periods=1:start_threshold=-50dB:start_silence=0.2";
        let af = format!("{sil},areverse,{sil},areverse,loudnorm=I=-14:TP=-1:LRA=11");
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("x").split('.').next().unwrap_or("x").to_string();
        let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let out = music.join(format!("{stem}.{ts}.{ext}"));
        let o = std::ffi::OsStr::new;
        ffmpeg(&[o("-i"), p.as_os_str(), o("-vn"), o("-af"), o(&af), o(codec[0]), o(codec[1]), o(codec[2]), o(codec[3]), out.as_os_str()])?;
        let dur = tool("ffprobe")
            .args(["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0"])
            .arg(&out)
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
            .map_or(0, |d| d.round() as u32);
        let _ = std::fs::remove_file(p);
        Ok(Processed { path: out.display().to_string(), dur })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Deserialize)]
struct ExportItem {
    path: String,
    title: String,
    artist: String,
    album: String,
    cover: Option<String>,
}

#[derive(serde::Serialize)]
struct ExportResult {
    done: usize,
    tagged: usize,
    failed: Vec<String>,
}

/// ตัวอักษรที่ Windows ห้ามใช้ในชื่อไฟล์ -> "_"
pub fn safe_name(s: &str) -> String {
    let t: String = s.chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let t = t.trim().trim_end_matches('.').trim();
    t.chars().take(120).collect::<String>()
}

/// คัดลอกทุกเพลงไปโฟลเดอร์ปลายทางเป็น "ศิลปิน - ชื่อเพลง.นามสกุล"; ถ้ามี ffmpeg ฝังแท็กและปกให้ (m4a/mp3)
#[tauri::command]
async fn export_library(dest: String, items: Vec<ExportItem>) -> Result<ExportResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dest = std::path::PathBuf::from(dest);
        std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        let have_ffmpeg = tool("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false);
        let (mut done, mut tagged, mut failed) = (0, 0, vec![]);
        for it in items {
            let src = std::path::Path::new(&it.path);
            let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("m4a").to_lowercase();
            let base = safe_name(&format!("{} - {}", it.artist, it.title));
            let mut out = dest.join(format!("{base}.{ext}"));
            for n in 2.. {
                if !out.exists() { break; }
                out = dest.join(format!("{base} ({n}).{ext}"));
            }
            let o = std::ffi::OsStr::new;
            let mut ok = false;
            if have_ffmpeg && (ext == "m4a" || ext == "mp3") {
                let (t, a, b) = (format!("title={}", it.title), format!("artist={}", it.artist), format!("album={}", it.album));
                let cover = it.cover.as_deref().filter(|c| !c.ends_with(".webp") && std::path::Path::new(c).exists());
                let mut args = vec![o("-i"), src.as_os_str()];
                if let Some(c) = cover { args.extend([o("-i"), o(c)]); }
                args.extend([o("-map"), o("0:a")]);
                if cover.is_some() { args.extend([o("-map"), o("1:v"), o("-disposition:v:0"), o("attached_pic")]); }
                args.extend([o("-c"), o("copy"), o("-metadata"), o(&t), o("-metadata"), o(&a), o("-metadata"), o(&b), out.as_os_str()]);
                ok = ffmpeg(&args).is_ok();
                if ok { tagged += 1; } else { let _ = std::fs::remove_file(&out); }
            }
            if ok || std::fs::copy(src, &out).is_ok() { done += 1; } else { failed.push(it.title); }
        }
        Ok(ExportResult { done, tagged, failed })
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- อัปเดตแอป: เช็กรีลีสล่าสุดบน GitHub แล้วโหลดตัวติดตั้งมารันให้ ----------

const RELEASES_API: &str = "https://api.github.com/repos/70d8/MySound/releases?per_page=20";
const RELEASE_DL_PREFIX: &str = "https://github.com/70d8/MySound/releases/download/";

#[derive(serde::Serialize, Debug, PartialEq)]
struct UpdateInfo {
    version: String,
    url: String,
    page: String,
}

fn ver_parts(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split('.').map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0)).collect()
}

/// เลือกรีลีสของแอป PC ที่ใหม่กว่าเวอร์ชันปัจจุบัน (แท็ก v<เลข>, ไม่ใช่ร่าง/pre-release, มีไฟล์ *_x64-setup.exe) ข้ามรีลีสของ Android (android-v*)
fn pick_update(releases: &serde_json::Value, current: &str) -> Option<UpdateInfo> {
    let cur = ver_parts(current);
    releases
        .as_array()?
        .iter()
        .filter(|r| r["draft"] != true && r["prerelease"] != true)
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            if !tag.strip_prefix('v')?.starts_with(|c: char| c.is_ascii_digit()) { return None; }
            let url = r["assets"].as_array()?.iter().find_map(|a| a["name"].as_str().filter(|n| n.ends_with("_x64-setup.exe")).and(a["browser_download_url"].as_str()))?;
            Some(UpdateInfo { version: tag.trim_start_matches('v').to_string(), url: url.to_string(), page: r["html_url"].as_str().unwrap_or("").to_string() })
        })
        .filter(|u| ver_parts(&u.version) > cur)
        .max_by_key(|u| ver_parts(&u.version))
}

#[tauri::command]
async fn check_update() -> Result<Option<UpdateInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut r = ureq::get(RELEASES_API)
            .header("User-Agent", "MySound-updater")
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| format!("เช็กเวอร์ชันใหม่ไม่ได้: {e}"))?;
        let bytes = r.body_mut().with_config().limit(5_000_000).read_to_vec().map_err(|e| e.to_string())?;
        let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Ok(pick_update(&v, env!("CARGO_PKG_VERSION")))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// โหลดตัวติดตั้งจาก GitHub Releases ของโปรเจกต์นี้เท่านั้น แล้วเปิดมัน และปิดแอปให้ตัวติดตั้งแทนที่ไฟล์ได้
#[tauri::command]
async fn install_update(app: AppHandle, url: String, silent: Option<bool>) -> Result<(), String> {
    if !url.starts_with(RELEASE_DL_PREFIX) || !url.ends_with("_x64-setup.exe") {
        return Err("ลิงก์อัปเดตไม่ถูกต้อง".into());
    }
    let path = tauri::async_runtime::spawn_blocking(move || -> Result<std::path::PathBuf, String> {
        let path = std::env::temp_dir().join("MySound-update-setup.exe");
        let mut r = ureq::get(&url).header("User-Agent", "MySound-updater").call().map_err(|e| format!("โหลดตัวติดตั้งไม่ได้: {e}"))?;
        let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        std::io::copy(&mut r.body_mut().with_config().limit(500_000_000).reader(), &mut f).map_err(|e| format!("โหลดตัวติดตั้งไม่สำเร็จ: {e}"))?;
        Ok(path)
    })
    .await
    .map_err(|e| e.to_string())??;
    if silent.unwrap_or(false) {
        // ติดตั้งเงียบ (/S) แล้วเปิดแอปใหม่เมื่อเสร็จ
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let script = format!("start \"\" /wait \"{}\" /S & start \"\" \"{}\"", path.display(), exe.display());
        let mut c = Command::new("cmd");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000);
            c.arg("/C").raw_arg(format!("\"{script}\"")); // ส่งข้อความตรงๆ ให้ cmd ไม่ให้ Rust ใส่ \" เพิ่ม
        }
        #[cfg(not(windows))]
        let _ = &script;
        c.spawn().map_err(|e| format!("เปิดตัวติดตั้งไม่ได้: {e}"))?;
    } else {
        Command::new(&path).spawn().map_err(|e| format!("เปิดตัวติดตั้งไม่ได้: {e}"))?;
    }
    app.exit(0);
    Ok(())
}

// ---------- พื้นที่เก็บข้อมูล / ซ่อมไฟล์ที่หาย ----------

#[derive(serde::Serialize)]
struct Storage {
    audio: u64,
    covers: u64,
    other: u64,
    library: u64,
    orphans: Vec<String>,
    orphan_bytes: u64,
}

const AUDIO_EXT: [&str; 9] = ["m4a", "mp3", "webm", "opus", "wav", "flac", "ogg", "aac", "mp4"];

/// ขนาดไฟล์ในโฟลเดอร์ music แยกประเภท + ไฟล์ที่ไม่มีเพลงไหนในคลังใช้แล้ว (keep = พาธที่ยังใช้อยู่)
#[tauri::command]
fn storage_info(app: AppHandle, keep: Vec<String>) -> Result<Storage, String> {
    let dir = data_dir(&app)?;
    let keep: std::collections::HashSet<String> = keep
        .iter()
        .filter_map(|p| std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().to_lowercase()))
        .collect();
    let mut s = Storage { audio: 0, covers: 0, other: 0, library: 0, orphans: vec![], orphan_bytes: 0 };
    s.library = std::fs::metadata(dir.join("library.json")).map(|m| m.len()).unwrap_or(0);
    for e in std::fs::read_dir(dir.join("music")).map_err(|e| e.to_string())?.flatten() {
        let Ok(m) = e.metadata() else { continue };
        if !m.is_file() { continue; }
        let name = e.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        let ext = lower.rsplit('.').next().unwrap_or("");
        if lower.contains(".cover.") || lower.starts_with("bg.") { s.covers += m.len(); } else if AUDIO_EXT.contains(&ext) { s.audio += m.len(); } else { s.other += m.len(); }
        if !keep.contains(&lower) && !lower.ends_with(".part") {
            s.orphans.push(e.path().display().to_string());
            s.orphan_bytes += m.len();
        }
    }
    Ok(s)
}

/// ค้นไฟล์ตามชื่อในโฟลเดอร์ที่ผู้ใช้เลือก (ลึกไม่เกิน 6 ชั้น) แล้วคัดลอกกลับเข้าโฟลเดอร์ music ของแอป
#[tauri::command]
async fn relink(app: AppHandle, dir: String, names: Vec<String>) -> Result<Vec<(String, String)>, String> {
    let music = data_dir(&app)?.join("music");
    tauri::async_runtime::spawn_blocking(move || {
        let mut want: std::collections::HashSet<String> = names.into_iter().map(|n| n.to_lowercase()).collect();
        let mut found = vec![];
        let mut stack = vec![(std::path::PathBuf::from(dir), 0u8)];
        while let Some((d, depth)) = stack.pop() {
            if want.is_empty() { break; }
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_dir() { if depth < 6 { stack.push((p, depth + 1)); } continue; }
                let name = e.file_name().to_string_lossy().to_string();
                if want.remove(&name.to_lowercase()) {
                    let dest = music.join(&name);
                    if !dest.exists() && std::fs::copy(&p, &dest).is_err() { continue; }
                    found.push((name, dest.display().to_string()));
                }
            }
        }
        Ok(found)
    })
    .await
    .map_err(|e| e.to_string())?
}

// ---------- แคชเนื้อเพลงบนดิสก์ (ใช้ออฟไลน์ได้) / ส่งออก .lrc ----------

#[tauri::command]
fn lyrics_cache_read(app: AppHandle) -> Result<String, String> {
    match std::fs::read_to_string(data_dir(&app)?.join("lyrics-cache.json")) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn lyrics_cache_write(app: AppHandle, json: String) -> Result<(), String> {
    let dir = data_dir(&app)?;
    let tmp = dir.join("lyrics-cache.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join("lyrics-cache.json")).map_err(|e| e.to_string())
}

/// เขียนไฟล์ .lrc ลงโฟลเดอร์ที่ผู้ใช้เลือก (ชื่อไฟล์ผ่าน safe_name, ไม่เขียนทับไฟล์ที่มีอยู่)
#[tauri::command]
fn write_lrc_files(dir: String, items: Vec<(String, String)>) -> Result<usize, String> {
    let dir = std::path::PathBuf::from(dir);
    if !dir.is_dir() { return Err("ไม่พบโฟลเดอร์ปลายทาง".into()); }
    let mut n = 0;
    for (name, text) in items {
        let base = safe_name(&name);
        if base.is_empty() { continue; }
        let mut p = dir.join(format!("{base}.lrc"));
        let mut k = 2;
        while p.exists() { p = dir.join(format!("{base} ({k}).lrc")); k += 1; }
        if std::fs::write(&p, text).is_ok() { n += 1; }
    }
    Ok(n)
}

// ---------- นำเข้าไฟล์เพลงจากเครื่อง (ลากมาวาง / Open with / โฟลเดอร์เฝ้าดู / M3U) ----------

#[derive(serde::Serialize)]
struct Imported {
    src: String,
    title: String,
    artist: String,
    album: String,
    dur: f64,
    path: String,
    cover: Option<String>,
    lrc: Option<String>,
}

fn is_audio(p: &std::path::Path) -> bool {
    p.extension().and_then(|e| e.to_str()).map(|e| AUDIO_EXT.contains(&e.to_lowercase().as_str())).unwrap_or(false)
}

/// อ่านความยาวและแท็ก (title/artist/album) ด้วย ffprobe; ไม่มี ffprobe ก็คืนค่าว่าง
fn probe(p: &std::path::Path) -> (f64, std::collections::HashMap<String, String>) {
    let Ok(out) = tool("ffprobe").args(["-v", "quiet", "-print_format", "json", "-show_format"]).arg(p).output() else { return (0.0, Default::default()) };
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    let dur = v["format"]["duration"].as_str().and_then(|d| d.parse().ok()).unwrap_or(0.0);
    let tags = v["format"]["tags"].as_object().map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.to_lowercase(), s.trim().to_string()))).collect()).unwrap_or_default();
    (dur, tags)
}

fn import_one(music: &std::path::Path, src: &std::path::Path, n: usize) -> Result<Imported, String> {
    if !src.is_file() || !is_audio(src) { return Err("not audio".into()); }
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("m4a").to_lowercase();
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let dest = if src.starts_with(music) {
        src.to_path_buf()
    } else {
        let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let d = music.join(format!("local-{millis}-{n}.{ext}"));
        std::fs::copy(src, &d).map_err(|e| e.to_string())?;
        d
    };
    let (dur, tags) = probe(&dest);
    let get = |k: &str| tags.get(k).filter(|v| !v.is_empty()).cloned();
    // ปกที่ฝังในไฟล์ (ถ้ามี)
    let cover_p = dest.with_extension("cover.jpg");
    let cover = ffmpeg(&[std::ffi::OsStr::new("-i"), dest.as_os_str(), std::ffi::OsStr::new("-an"), std::ffi::OsStr::new("-frames:v"), std::ffi::OsStr::new("1"), cover_p.as_os_str()])
        .ok()
        .filter(|_| cover_p.is_file())
        .map(|_| cover_p.display().to_string());
    // ไฟล์ .lrc ชื่อเดียวกันวางข้างไฟล์เพลง
    let lrc = std::fs::read_to_string(src.with_extension("lrc")).ok().filter(|t| t.len() < 1_000_000);
    Ok(Imported {
        src: src.display().to_string(),
        title: get("title").unwrap_or(stem),
        artist: get("artist").or_else(|| get("album_artist")).unwrap_or_default(),
        album: get("album").unwrap_or_default(),
        dur,
        path: dest.display().to_string(),
        cover,
        lrc,
    })
}

#[tauri::command]
async fn import_files(app: AppHandle, paths: Vec<String>) -> Result<Vec<Imported>, String> {
    let music = data_dir(&app)?.join("music");
    tauri::async_runtime::spawn_blocking(move || {
        Ok(paths.iter().enumerate().filter_map(|(i, p)| import_one(&music, std::path::Path::new(p), i).ok()).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// รายชื่อไฟล์เสียงในโฟลเดอร์ (ลึกไม่เกิน 4 ชั้น, สูงสุด 5000 ไฟล์)
#[tauri::command]
fn scan_folder(dir: String) -> Vec<String> {
    let mut out = vec![];
    let mut stack = vec![(PathBuf::from(dir), 0u8)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { if depth < 4 { stack.push((p, depth + 1)); } } else if is_audio(&p) { out.push(p.display().to_string()); }
            if out.len() >= 5000 { return out; }
        }
    }
    out
}

/// เขียนไฟล์ข้อความที่ผู้ใช้เลือกที่เก็บเอง (สำรองคลัง / เพลย์ลิสต์) จำกัดนามสกุล
#[tauri::command]
fn save_text(path: String, text: String) -> Result<(), String> {
    let ext = std::path::Path::new(&path).extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !["json", "m3u", "m3u8", "mysound", "txt", "csv"].contains(&ext.as_str()) { return Err("นามสกุลไฟล์นี้ไม่รองรับ".into()); }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_text(path: String) -> Result<String, String> {
    let p = std::path::Path::new(&path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !["json", "m3u", "m3u8", "mysound", "txt", "csv", "lrc"].contains(&ext.as_str()) { return Err("นามสกุลไฟล์นี้ไม่รองรับ".into()); }
    if std::fs::metadata(p).map_err(|e| e.to_string())?.len() > 50_000_000 { return Err("ไฟล์ใหญ่เกินไป".into()); }
    let b = std::fs::read(p).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&b)).into_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn library_fallback() {
        assert_eq!(super::pick_library("", Some("{\"a\":1}")), "");
        assert_eq!(super::pick_library("{\"tracks\":[]}", Some("{}")), "{\"tracks\":[]}");
        assert_eq!(super::pick_library("{\"tracks\":[", Some("{\"tracks\":[1]}")), "{\"tracks\":[1]}");
        assert_eq!(super::pick_library("{\"tracks\":[", Some("oops")), "{\"tracks\":[");
    }

    #[test]
    fn safe_names() {
        assert_eq!(super::safe_name("AC/DC: Back in Black?"), "AC_DC_ Back in Black_");
        assert_eq!(super::safe_name("  end.. "), "end");
    }
    #[test]
    fn pick_update_rules() {
        let j = serde_json::json!([
            {"tag_name":"android-v1.0.0","draft":false,"prerelease":false,"html_url":"p0","assets":[{"name":"a.apk","browser_download_url":"u0"}]},
            {"tag_name":"v0.1.9","draft":true,"prerelease":false,"html_url":"p9","assets":[{"name":"MySound_0.1.9_x64-setup.exe","browser_download_url":"u9"}]},
            {"tag_name":"v0.1.3","draft":false,"prerelease":false,"html_url":"p3","assets":[{"name":"MySound_0.1.3_x64-setup.exe","browser_download_url":"u3"}]},
            {"tag_name":"v0.1.10","draft":false,"prerelease":false,"html_url":"p10","assets":[{"name":"MySound_0.1.10_x64-setup.exe","browser_download_url":"u10"}]},
            {"tag_name":"v0.1.11","draft":false,"prerelease":false,"html_url":"p11","assets":[]}
        ]);
        // เลขสองหลักต้องมากกว่า 3 / ไม่เอาร่าง / ไม่เอา android / ไม่เอารีลีสที่ไม่มี exe
        assert_eq!(super::pick_update(&j, "0.1.2").map(|u| u.version), Some("0.1.10".into()));
        assert_eq!(super::pick_update(&j, "0.1.10"), None);
        assert_eq!(super::pick_update(&serde_json::json!({}), "0.1.0"), None);
    }
}


#[derive(serde::Serialize, Default)]
struct Lyrics {
    synced: Option<String>,
    plain: Option<String>,
}

fn lrclib_json(url: &str) -> Result<Option<serde_json::Value>, String> {
    // ต้องมี timeout ไม่งั้นถ้า LRCLIB ช้าจะค้างไม่รู้จบ; สะดุดครั้งแรกลองอีกรอบ
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(7))).build().into();
    let mut last = String::new();
    for _ in 0..2 {
        match agent.get(url).header("User-Agent", "MySound/0.1 (personal player)").call() {
            Ok(mut r) => {
                let b = r.body_mut().with_config().limit(2_000_000).read_to_vec().map_err(|e| e.to_string())?;
                return Ok(serde_json::from_slice(&b).ok());
            }
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(ureq::Error::StatusCode(c)) if c < 500 => return Err(format!("ค้นเนื้อเพลงไม่ได้: HTTP {c}")),
            Err(e) => last = format!("ค้นเนื้อเพลงไม่ได้: {e}"),
        }
    }
    Err(last)
}

fn lyrics_of(v: &serde_json::Value) -> Lyrics {
    let f = |k: &str| v[k].as_str().filter(|s| !s.trim().is_empty()).map(String::from);
    Lyrics { synced: f("syncedLyrics"), plain: f("plainLyrics") }
}

/// เนื้อเพลงจาก LRCLIB (ฟรี ไม่ต้องมี key): จับคู่ตรงตัวกับค้นด้วยชื่อที่ล้างแล้ว "ยิงพร้อมกัน" (เดิมรอทีละอัน)
/// ถ้าจับคู่ตรงได้ก็ใช้ตัวนั้น ไม่งั้นใช้ผลค้นหา
#[tauri::command]
async fn lyrics(title: String, artist: String, dur: u32) -> Result<Lyrics, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let enc = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
        let exact = format!("https://lrclib.net/api/get?track_name={}&artist_name={}&duration={dur}", enc(&title), enc(&artist));
        let a = if artist == "ไม่ทราบศิลปิน" { "" } else { artist.as_str() };
        let search = format!("https://lrclib.net/api/search?q={}", enc(&ytdlp::clean_query(&title, a)));
        let h = std::thread::spawn(move || lrclib_json(&search));
        let ex = lrclib_json(&exact);
        let se = h.join().map_err(|_| "ค้นเนื้อเพลงไม่ได้".to_string())?;
        if let Ok(Some(v)) = &ex {
            let l = lyrics_of(v);
            if l.synced.is_some() || l.plain.is_some() { return Ok(l); }
        }
        let list = match se {
            Ok(Some(serde_json::Value::Array(l))) => l,
            Ok(_) => { return ex.map(|_| Lyrics::default()); } // ค้นไม่เจอจริง (ถ้าจับคู่ตรงพังเพราะเน็ต ก็ส่ง error ต่อ)
            Err(e) => return Err(e),
        };
        // ชอบเพลงที่ความยาวใกล้กัน (ต่างไม่เกิน 5 วินาที) และมี synced
        let near = |v: &&serde_json::Value| v["duration"].as_f64().map_or(true, |d| (d - dur as f64).abs() <= 5.0);
        let best = list.iter().filter(near).find(|v| lyrics_of(v).synced.is_some()).or_else(|| list.iter().filter(near).next());
        Ok(best.map(lyrics_of).unwrap_or_default())
    })
    .await
    .map_err(|e| e.to_string())?
}


// ---------- Discord Rich Presence ----------
// Application ID ไม่ใช่ความลับ (ใช้แสดงชื่อ "MySound" ในสถานะ Discord)
const DISCORD_APP_ID: &str = "1573766901916762202";
const NO_DISCORD: &str = "ต่อ Discord ไม่ได้ ต้องเปิด Discord แบบแอปบนคอมไว้ก่อน";

#[derive(Default)]
struct Discord(std::sync::Arc<Mutex<Option<DiscordIpcClient>>>);

/// Discord รับข้อความยาว 2-128 ตัวอักษรเท่านั้น
fn dtext(s: &str) -> Option<String> {
    let t: String = s.trim().chars().take(128).collect();
    (t.chars().count() >= 2).then_some(t)
}

/// ชื่อที่สั้นเกิน 2 ตัวอักษร Discord จะปฏิเสธ จึงเติมสัญลักษณ์นำหน้าแทนการไม่ส่ง
fn dtitle(s: &str) -> String {
    dtext(s).or_else(|| dtext(&format!("♪ {}", s.trim()))).unwrap_or_else(|| "MySound".into())
}

#[tauri::command]
async fn discord_update(state: tauri::State<'_, Discord>, title: String, artist: String, playing: bool, pos: f64, dur: f64, cover: Option<String>, line: Option<String>, url: Option<String>) -> Result<(), String> {
    let cell = state.0.clone();
    // ต่อ/ส่งผ่าน IPC เป็นงาน blocking: ย้ายไป thread แยก ไม่ให้ค้าง runtime ถ้า Discord ตอบช้า
    tauri::async_runtime::spawn_blocking(move || {
        let mut g = cell.lock().map_err(|e| e.to_string())?;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
        // ลองสูงสุด 2 รอบ: รอบแรกพลาด (Discord เพิ่งเปิด/ปิดไปแล้วเปิดใหม่) ก็ต่อใหม่แล้วส่งซ้ำทันที
        for attempt in 0..2 {
            if g.is_none() {
                let mut c = DiscordIpcClient::new(DISCORD_APP_ID);
                if c.connect().is_err() {
                    return Err(NO_DISCORD.to_string());
                }
                *g = Some(c);
            }
            let details = dtitle(&title);
            let mut a = Activity::new().activity_type(ActivityType::Listening).details(details);
            if let Some(u) = cover.as_deref().filter(|u| u.starts_with("https://") && u.len() < 256) {
                let mut assets = Assets::new().large_image(u);
                if line.is_some() { if let Some(t) = dtext(&artist) { assets = assets.large_text(t); } } // ศิลปินย้ายไปแสดงตอนชี้รูปปก
                a = a.assets(assets);
            }
            // ปุ่มใต้สถานะ: เปิดเพลงนี้บน YouTube/SoundCloud (หรือค้นบน YouTube ถ้าเป็นไฟล์ในเครื่อง)
            let link = url.clone().filter(|u| u.starts_with("https://") && u.len() < 500)
                .unwrap_or_else(|| format!("https://www.youtube.com/results?search_query={}", url::form_urlencoded::byte_serialize(format!("{artist} {title}").as_bytes()).collect::<String>()));
            let label = if link.contains("soundcloud.com") { "Listen on SoundCloud" } else if link.contains("/results?") { "Search on YouTube" } else { "Listen on YouTube" };
            a = a.buttons(vec![discord_rich_presence::activity::Button::new(label, link)]);
            if playing {
                // มีบรรทัดเนื้อเพลง: ใช้แทนชื่อศิลปินในบรรทัดที่สอง
                if let Some(t) = line.as_deref().and_then(dtext).or_else(|| dtext(&artist)) { a = a.state(t); }
                let start = now - (pos * 1000.0) as i64;
                let mut ts = Timestamps::new().start(start);
                if dur > 0.0 { ts = ts.end(start + (dur * 1000.0) as i64); }
                a = a.timestamps(ts);
            } else {
                a = a.state("Paused");
            }
            if g.as_mut().map(|c| c.set_activity(a)).transpose().is_ok() {
                return Ok(());
            }
            *g = None; // ท่อเสีย: ทิ้งแล้วต่อใหม่ในรอบถัดไป
            if attempt == 1 { break; }
        }
        Err(NO_DISCORD.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn discord_clear(state: tauri::State<'_, Discord>) -> Result<(), String> {
    let cell = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut g = cell.lock().map_err(|e| e.to_string())?;
        if let Some(c) = g.as_mut() {
            if c.clear_activity().is_err() { *g = None; }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}


/// ค้นเพลงบน YouTube / SoundCloud ผ่าน yt-dlp (ไม่โหลดอะไร แค่ขอรายชื่อ)
#[tauri::command]
async fn search_online(query: String, source: String) -> Result<Vec<ytdlp::Hit>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let q: String = query.chars().filter(|c| !c.is_control()).collect();
        if q.trim().is_empty() {
            return Ok(vec![]);
        }
        let prefix = if source == "soundcloud" { "scsearch10" } else { "ytsearch10" };
        let out = tool("yt-dlp")
            .args(["--flat-playlist", "-j", "--no-warnings", "--encoding", "utf-8"])
            .arg(format!("{prefix}:{}", q.trim()))
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "ไม่พบ yt-dlp ติดตั้งด้วย winget install yt-dlp.yt-dlp แล้วปิดและเปิดแอปใหม่".to_string()
                } else {
                    e.to_string()
                }
            })?;
        let hits: Vec<_> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .filter_map(|v| ytdlp::parse_hit(&v))
            .collect();
        if hits.is_empty() && !out.status.success() {
            let e = String::from_utf8_lossy(&out.stderr).lines().rev().find(|l| l.contains("ERROR")).unwrap_or("ค้นหาไม่สำเร็จ").to_string();
            return Err(e);
        }
        Ok(hits)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// อ่านไฟล์เนื้อเพลง (.lrc / .txt) ขนาดไม่เกิน 1 MB
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let p = std::path::Path::new(&path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext != "lrc" && ext != "txt" {
        return Err("รองรับเฉพาะไฟล์ .lrc และ .txt".into());
    }
    if std::fs::metadata(p).map_err(|e| e.to_string())?.len() > 1_000_000 {
        return Err("ไฟล์ใหญ่เกินไป".into());
    }
    std::fs::read(p).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // เปิดแอปซ้ำ: ไม่เปิดหน้าต่างใหม่ แค่ดึงหน้าต่างเดิมขึ้นมา (และส่งไฟล์ที่สั่งเปิดไปให้)
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            show_main(app);
            let files: Vec<String> = argv.into_iter().skip(1).filter(|a| !a.starts_with("--")).collect();
            if !files.is_empty() { let _ = app.emit("open-files", files); }
        }))
        .manage(Discord::default())
        .manage(extra::Remote::default())
        .manage(extra::Whisper::default())
        .manage(extra::WSetup::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            build_tray(app.handle())?;
            if let Some(w) = app.get_webview_window("main") { let _ = w.set_icon(app_icon()?); }
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(w) = app.get_webview_window("main") { let _ = w.hide(); }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![read_library, write_library, download, genius_cover, import_cover, missing_files, delete_files, tool_version, ytdlp_update, process_audio, export_library, lyrics, discord_update, discord_clear, check_update, install_update, storage_info, relink, lyrics_cache_read, lyrics_cache_write, write_lrc_files, search_online, read_text_file, launch_files, tray_tooltip, import_files, scan_folder, save_text, load_text, extra::open_url, extra::save_png, extra::yt_info, extra::cut_audio, extra::convert_mp3, extra::import_bg_video, extra::lastfm_token, extra::lastfm_session, extra::lastfm_track, extra::remote_start, extra::remote_stop, extra::remote_push, extra::whisper_state, extra::whisper_setup, extra::whisper_setup_cancel, extra::whisper_run, extra::whisper_cancel, extra::pet_import, extra::pet_bytes, spotify::spotify_status, spotify::spotify_logout, spotify::spotify_login, spotify::spotify_playlists, spotify::spotify_tracks])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// ไฟล์ที่ส่งมาตอนเปิดแอป (เช่น คลิกขวาไฟล์เพลง > Open with MySound)
#[tauri::command]
fn launch_files() -> Vec<String> {
    std::env::args().skip(1).filter(|a| !a.starts_with("--")).collect()
}

/// ไอคอนแอปฝังด้วย include_bytes: ไฟล์ไอคอนเปลี่ยนเมื่อไร cargo คอมไพล์ใหม่ให้เอง (ไม่ค้างไอคอนเก่าใน dev)
fn app_icon() -> tauri::Result<tauri::image::Image<'static>> {
    tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))
}

/// ไอคอนถาดระบบ: คลิกซ้าย = เปิดหน้าต่าง, คลิกขวา = เมนูควบคุมเพลง
fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let item = |id: &str, label: &str| MenuItem::with_id(app, id, label, true, None::<&str>);
    let menu = Menu::with_items(app, &[
        &item("show", "Open MySound")?,
        &PredefinedMenuItem::separator(app)?,
        &item("toggle", "Play / Pause")?,
        &item("next", "Next")?,
        &item("prev", "Previous")?,
        &PredefinedMenuItem::separator(app)?,
        &item("quit", "Quit")?,
    ])?;
    let b = TrayIconBuilder::with_id("main").tooltip("MySound").menu(&menu).show_menu_on_left_click(false).icon(app_icon()?);
    b.on_menu_event(|app, e| match e.id().as_ref() {
        "show" => show_main(app),
        "quit" => app.exit(0),
        cmd => { let _ = app.emit("np-cmd", serde_json::json!({ "cmd": cmd })); }
    })
    .on_tray_icon_event(|tray, e| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            show_main(tray.app_handle());
        }
    })
    .build(app)?;
    Ok(())
}

/// เปลี่ยนข้อความ tooltip ของไอคอนถาดระบบเป็นชื่อเพลงที่เล่นอยู่
#[tauri::command]
fn tray_tooltip(app: AppHandle, text: String) {
    if let Some(t) = app.tray_by_id("main") { let _ = t.set_tooltip(Some(text.chars().take(120).collect::<String>())); }
}
