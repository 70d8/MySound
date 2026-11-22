mod ytdlp;

use discord_rich_presence::{activity::{Activity, ActivityType, Assets, Timestamps}, DiscordIpc, DiscordIpcClient};
use std::sync::Mutex;

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter, Manager};
use ytdlp::Line;

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(d.join("music")).map_err(|e| e.to_string())?;
    Ok(d)
}

/// คืนข้อความ JSON ของคลังเพลง (ว่างถ้ายังไม่เคยบันทึก)
#[tauri::command]
fn read_library(app: AppHandle) -> Result<String, String> {
    let p = data_dir(&app)?.join("library.json");
    match std::fs::read_to_string(&p) {
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.to_string()),
    }
}

/// เขียนไฟล์ชั่วคราวก่อนแล้วค่อย rename กันไฟล์พังถ้าปิดแอปกลางคัน
#[tauri::command]
fn write_library(app: AppHandle, json: String) -> Result<(), String> {
    let dir = data_dir(&app)?;
    let tmp = dir.join("library.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join("library.json")).map_err(|e| e.to_string())
}

#[derive(Clone, serde::Serialize)]
struct Progress {
    id: u64,
    pct: f32,
    title: Option<String>,
    state: &'static str,
}

fn run_download(app: &AppHandle, id: u64, url: &str, playlist: bool) -> Result<usize, String> {
    let music = data_dir(app)?.join("music");
    let mut cmd = Command::new(tool_path("yt-dlp"));
    cmd.args([if playlist { "--yes-playlist" } else { "--no-playlist" }, "--newline", "--progress", "--encoding", "utf-8"])
        // เสียงอย่างเดียว เลือก m4a ก่อน (เล่นได้ใน WebView2 โดยไม่ต้องใช้ ffmpeg)
        .args(["-f", "bestaudio[ext=m4a]/bestaudio[ext=mp3]/bestaudio"])
        .args(["-o", &format!("{}/%(id)s.%(ext)s", music.display())])
        .args(["--progress-template", &format!("download:{} %(progress._percent_str)s", ytdlp::P_PROGRESS)])
        .args(["--print", &format!("before_dl:{}\t%(title)s", ytdlp::P_TITLE)])
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
        BufReader::new(err_pipe).lines().map_while(Result::ok).filter(|l| l.contains("ERROR")).last()
    });

    let emit = |pct, title: Option<String>, state| {
        let _ = app.emit("dl-progress", Progress { id, pct, title, state });
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
    let last_err = err_thread.join().ok().flatten();
    if count > 0 {
        Ok(count) // ถ้ามีบางเพลงในเพลย์ลิสต์พัง แต่ได้อย่างน้อย 1 เพลงก็ถือว่าสำเร็จ
    } else {
        let _ = status;
        Err(last_err.unwrap_or_else(|| "ดาวน์โหลดไม่สำเร็จ".into()))
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
async fn download(app: AppHandle, id: u64, url: String, playlist: bool) -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || run_download(&app, id, &url, playlist))
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

#[cfg(test)]
mod tests {
    #[test]
    fn safe_names() {
        assert_eq!(super::safe_name("AC/DC: Back in Black?"), "AC_DC_ Back in Black_");
        assert_eq!(super::safe_name("  end.. "), "end");
    }
}


#[derive(serde::Serialize, Default)]
struct Lyrics {
    synced: Option<String>,
    plain: Option<String>,
}

fn lrclib_json(url: &str) -> Result<Option<serde_json::Value>, String> {
    match ureq::get(url).header("User-Agent", "MySound/0.1 (personal player)").call() {
        Ok(mut r) => {
            let b = r.body_mut().with_config().limit(2_000_000).read_to_vec().map_err(|e| e.to_string())?;
            Ok(serde_json::from_slice(&b).ok())
        }
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(e) => Err(format!("ค้นเนื้อเพลงไม่ได้: {e}")),
    }
}

fn lyrics_of(v: &serde_json::Value) -> Lyrics {
    let f = |k: &str| v[k].as_str().filter(|s| !s.trim().is_empty()).map(String::from);
    Lyrics { synced: f("syncedLyrics"), plain: f("plainLyrics") }
}

/// เนื้อเพลงจาก LRCLIB (ฟรี ไม่ต้องมี key): ลองจับคู่ตรงตัวก่อน ไม่เจอค่อยค้นด้วยชื่อที่ล้างแล้ว
#[tauri::command]
async fn lyrics(title: String, artist: String, dur: u32) -> Result<Lyrics, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let enc = |s: &str| url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>();
        let exact = format!("https://lrclib.net/api/get?track_name={}&artist_name={}&duration={dur}", enc(&title), enc(&artist));
        if let Some(v) = lrclib_json(&exact)? {
            let l = lyrics_of(&v);
            if l.synced.is_some() || l.plain.is_some() {
                return Ok(l);
            }
        }
        let a = if artist == "ไม่ทราบศิลปิน" { "" } else { artist.as_str() };
        let q = ytdlp::clean_query(&title, a);
        let Some(serde_json::Value::Array(list)) = lrclib_json(&format!("https://lrclib.net/api/search?q={}", enc(&q)))? else {
            return Ok(Lyrics::default());
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
async fn discord_update(state: tauri::State<'_, Discord>, title: String, artist: String, playing: bool, pos: f64, dur: f64, cover: Option<String>, line: Option<String>) -> Result<(), String> {
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
        .manage(Discord::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![read_library, write_library, download, genius_cover, import_cover, missing_files, delete_files, tool_version, ytdlp_update, process_audio, export_library, lyrics, discord_update, discord_clear, search_online, read_text_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
