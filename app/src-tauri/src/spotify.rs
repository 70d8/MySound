//! เชื่อม Spotify (OAuth PKCE ด้วย Client ID ของผู้ใช้เอง) เพื่ออ่าน “รายชื่อเพลง” ของเพลย์ลิสต์/อัลบั้ม
//! ไม่ได้โหลดเสียงจาก Spotify: หน้าจอจะเอารายชื่อไปค้นหาเพลงเดียวกันบน YouTube แล้วโหลดด้วยระบบเดิม
//! ข้อจำกัดของ Spotify (ก.พ. 2026): เจ้าของแอปต้องมี Premium, อ่านเพลงได้เฉพาะเพลย์ลิสต์ที่ผู้ใช้เป็นเจ้าของ/ร่วมแก้ไข
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

pub const PORT: u16 = 43827;
pub const REDIRECT: &str = "http://127.0.0.1:43827/callback";
const SCOPES: &str = "playlist-read-private playlist-read-collaborative user-library-read";
const API: &str = "https://api.spotify.com/v1";

#[derive(Serialize, Deserialize, Default, Clone)]
struct Tok { client_id: String, refresh: String, access: String, exp: u64, user: String }

#[derive(Serialize)]
pub struct SpPlaylist { id: String, name: String, count: u64, image: Option<String>, mine: bool }
#[derive(Serialize)]
pub struct SpTrack { title: String, artist: String, album: String, dur: u32 }
#[derive(Serialize)]
pub struct SpList { name: String, tracks: Vec<SpTrack> }

fn file(app: &AppHandle) -> Result<PathBuf, String> {
    let d = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d.join("spotify.json"))
}
fn load(app: &AppHandle) -> Option<Tok> { serde_json::from_str(&std::fs::read_to_string(file(app).ok()?).ok()?).ok() }
fn save(app: &AppHandle, t: &Tok) -> Result<(), String> { std::fs::write(file(app)?, serde_json::to_string(t).unwrap_or_default()).map_err(|e| e.to_string()) }
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) }
fn b64(b: &[u8]) -> String { base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b) }
fn random(n: usize) -> Result<String, String> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| e.to_string())?;
    Ok(b64(&buf))
}
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(20))).http_status_as_error(false).build().into()
}
fn api_err(v: &Value, status: u16) -> String {
    let m = v["error_description"].as_str().or(v["error"]["message"].as_str()).or(v["error"].as_str()).unwrap_or("");
    format!("Spotify ตอบกลับผิดพลาด ({status}) {m}").trim().to_string()
}

fn token_req(form: &[(&str, &str)]) -> Result<Value, String> {
    let mut r = agent().post("https://accounts.spotify.com/api/token").send_form(form.iter().copied()).map_err(|e| e.to_string())?;
    let st = r.status().as_u16();
    let v: Value = r.body_mut().read_to_string().ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
    if st != 200 { return Err(api_err(&v, st)); }
    Ok(v)
}

/// คืน access token ที่ยังใช้ได้ (ต่ออายุเองถ้าใกล้หมด)
fn access(app: &AppHandle, force: bool) -> Result<String, String> {
    let mut t = load(app).ok_or("ยังไม่ได้เชื่อม Spotify (ตั้งค่า → เชื่อมต่อ)")?;
    if !force && t.exp > now() + 60 { return Ok(t.access); }
    let v = token_req(&[("grant_type", "refresh_token"), ("refresh_token", &t.refresh), ("client_id", &t.client_id)])
        .map_err(|e| format!("ต้องเชื่อม Spotify ใหม่ ({e})"))?;
    t.access = v["access_token"].as_str().ok_or("ต่ออายุไม่สำเร็จ")?.to_string();
    if let Some(r) = v["refresh_token"].as_str() { t.refresh = r.to_string(); }
    t.exp = now() + v["expires_in"].as_u64().unwrap_or(3600);
    save(app, &t)?;
    Ok(t.access)
}

fn get(app: &AppHandle, url: &str) -> Result<Value, String> {
    let mut tok = access(app, false)?;
    for attempt in 0..4 {
        let mut r = agent().get(url).header("Authorization", &format!("Bearer {tok}")).call().map_err(|e| e.to_string())?;
        let st = r.status().as_u16();
        let wait = r.headers().get("retry-after").and_then(|h| h.to_str().ok()).and_then(|s| s.parse::<u64>().ok()).unwrap_or(2);
        let v: Value = r.body_mut().read_to_string().ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        match st {
            200 => return Ok(v),
            401 if attempt == 0 => tok = access(app, true)?,
            403 => return Err("Spotify ไม่ให้อ่านรายการนี้: อ่านได้เฉพาะเพลย์ลิสต์ที่คุณสร้างหรือร่วมแก้ไข (เพลย์ลิสต์ของคนอื่น ให้ก๊อปเพลงไปไว้ในเพลย์ลิสต์ของคุณก่อน) และบัญชีเจ้าของ Client ID ต้องเป็น Premium".into()),
            404 => return Err("ไม่พบเพลย์ลิสต์/อัลบั้มนี้ใน Spotify".into()),
            429 => std::thread::sleep(Duration::from_secs(wait.min(20))),
            _ => return Err(api_err(&v, st)),
        }
    }
    Err("Spotify ไม่ตอบ ลองใหม่อีกครั้ง".into())
}

/// เดินทุกหน้า (ช่อง next) แล้วรวม items
fn pages(app: &AppHandle, first: &str, max: usize) -> Result<Vec<Value>, String> {
    let (mut out, mut url) = (Vec::new(), Some(first.to_string()));
    while let Some(u) = url.take() {
        let v = get(app, &u)?;
        out.extend(v["items"].as_array().cloned().unwrap_or_default());
        if out.len() >= max { break; }
        url = v["next"].as_str().filter(|n| n.starts_with(API)).map(String::from);
    }
    Ok(out)
}

fn track(t: &Value, album: Option<&str>) -> Option<SpTrack> {
    if t.is_null() || t["type"].as_str() == Some("episode") { return None; }
    let title = t["name"].as_str().filter(|s| !s.trim().is_empty())?.to_string();
    let artist = t["artists"].as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    let album = album.map(String::from).or_else(|| t["album"]["name"].as_str().map(String::from)).unwrap_or_default();
    Some(SpTrack { title, artist, album, dur: (t["duration_ms"].as_u64().unwrap_or(0) / 1000) as u32 })
}

/// แยกชนิด + id จากลิงก์/URI ของ Spotify: ("playlist"|"album", id)
pub fn parse_link(s: &str) -> Option<(&'static str, String)> {
    let s = s.trim();
    let rest = s.strip_prefix("spotify:").map(|r| r.replace(':', "/")).or_else(|| {
        let u = url::Url::parse(s).ok()?;
        if u.host_str()? != "open.spotify.com" { return None; }
        Some(u.path().trim_start_matches('/').to_string())
    })?;
    let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty() && !p.starts_with("intl-")).collect();
    let kind = match parts.first()? { &"playlist" => "playlist", &"album" => "album", _ => return None };
    let id = parts.get(1)?.split('?').next()?.to_string();
    (id.len() >= 10 && id.chars().all(|c| c.is_ascii_alphanumeric())).then_some((kind, id))
}

#[tauri::command]
pub fn spotify_status(app: AppHandle) -> Option<String> { load(&app).map(|t| t.user) }

#[tauri::command]
pub fn spotify_logout(app: AppHandle) -> Result<(), String> {
    let f = file(&app)?;
    if f.exists() { std::fs::remove_file(f).map_err(|e| e.to_string())?; }
    Ok(())
}

/// เปิดเบราว์เซอร์ให้ล็อกอิน Spotify แล้วรอรับรหัสที่ 127.0.0.1:43827 (รอสูงสุด 3 นาที)
#[tauri::command]
pub async fn spotify_login(app: AppHandle, client_id: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let cid = client_id.trim().to_string();
        if cid.len() != 32 || !cid.chars().all(|c| c.is_ascii_hexdigit()) { return Err("Client ID ต้องเป็นตัวอักษร 32 ตัว (คัดลอกจากหน้า Dashboard ของแอปใน Spotify for Developers)".into()); }
        let verifier = random(64)?;
        let challenge = b64(&Sha256::digest(verifier.as_bytes()));
        let state = random(16)?;
        let server = tiny_http::Server::http(("127.0.0.1", PORT)).map_err(|_| format!("พอร์ต {PORT} ถูกโปรแกรมอื่นใช้อยู่ ปิดโปรแกรมนั้นหรือรีสตาร์ทเครื่องแล้วลองใหม่"))?;
        let mut auth = url::Url::parse("https://accounts.spotify.com/authorize").unwrap();
        auth.query_pairs_mut().append_pair("client_id", &cid).append_pair("response_type", "code").append_pair("redirect_uri", REDIRECT)
            .append_pair("code_challenge_method", "S256").append_pair("code_challenge", &challenge).append_pair("scope", SCOPES).append_pair("state", &state);
        crate::extra::open_url(auth.to_string())?;
        let deadline = Instant::now() + Duration::from_secs(180);
        let code = loop {
            if Instant::now() > deadline { return Err("หมดเวลารอ ลองกดเชื่อมใหม่อีกครั้ง".into()); }
            let Ok(Some(req)) = server.recv_timeout(Duration::from_millis(500)) else { continue };
            let u = url::Url::parse(&format!("http://127.0.0.1{}", req.url())).ok();
            if u.as_ref().map(|u| u.path()) != Some("/callback") { let _ = req.respond(tiny_http::Response::empty(404)); continue; }
            let q: std::collections::HashMap<String, String> = u.unwrap().query_pairs().into_owned().collect();
            let ok = q.get("state") == Some(&state) && q.contains_key("code");
            let html = if ok { "<h2>เชื่อม Spotify กับ MySound แล้ว</h2><p>ปิดหน้านี้แล้วกลับไปที่แอปได้เลย</p>" } else { "<h2>เชื่อมไม่สำเร็จ</h2><p>กลับไปที่แอปแล้วลองใหม่</p>" };
            let page = format!("<!doctype html><meta charset=utf-8><body style='font-family:sans-serif;background:#111;color:#eee;text-align:center;padding-top:15vh'>{html}</body>");
            let _ = req.respond(tiny_http::Response::from_string(page).with_header("Content-Type: text/html; charset=utf-8".parse::<tiny_http::Header>().unwrap()));
            if let Some(e) = q.get("error") { return Err(if e == "access_denied" { "ยกเลิกการเชื่อม Spotify".into() } else { format!("Spotify: {e}") }); }
            if !ok { return Err("ข้อมูลยืนยันไม่ตรง ลองใหม่อีกครั้ง".into()); }
            break q["code"].clone();
        };
        let v = token_req(&[("grant_type", "authorization_code"), ("code", &code), ("redirect_uri", REDIRECT), ("client_id", &cid), ("code_verifier", &verifier)])
            .map_err(|e| if e.contains("redirect") { format!("{e} · ตรวจว่าใส่ Redirect URI เป็น {REDIRECT} ในหน้าแอปของ Spotify") } else { e })?;
        let mut t = Tok {
            client_id: cid,
            refresh: v["refresh_token"].as_str().unwrap_or_default().to_string(),
            access: v["access_token"].as_str().ok_or("ไม่ได้รับ token จาก Spotify")?.to_string(),
            exp: now() + v["expires_in"].as_u64().unwrap_or(3600),
            user: String::new(),
        };
        save(&app, &t)?;
        let me = get(&app, &format!("{API}/me")).map_err(|e| if e.contains("Premium") { "Spotify ไม่อนุญาต: บัญชีเจ้าของ Client ID ต้องเป็น Premium และต้องเพิ่มอีเมลบัญชีนี้ไว้ใน User Management ของแอป".to_string() } else { e });
        let me = match me { Ok(v) => v, Err(e) => { let _ = spotify_logout(app.clone()); return Err(e); } };
        t.user = me["display_name"].as_str().or(me["id"].as_str()).unwrap_or("Spotify").to_string();
        save(&app, &t)?;
        Ok(t.user)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// เพลย์ลิสต์ของผู้ใช้ (mine = เป็นเจ้าของ/ร่วมแก้ไข ซึ่ง Spotify ให้อ่านรายชื่อเพลงได้)
#[tauri::command]
pub async fn spotify_playlists(app: AppHandle) -> Result<Vec<SpPlaylist>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let me = get(&app, &format!("{API}/me"))?;
        let uid = me["id"].as_str().unwrap_or_default().to_string();
        Ok(pages(&app, &format!("{API}/me/playlists?limit=50"), 1000)?.iter().filter(|p| !p.is_null()).map(|p| SpPlaylist {
            id: p["id"].as_str().unwrap_or_default().to_string(),
            name: p["name"].as_str().unwrap_or("?").to_string(),
            count: p["items"]["total"].as_u64().or(p["tracks"]["total"].as_u64()).unwrap_or(0),
            image: p["images"].as_array().and_then(|a| a.last()).and_then(|i| i["url"].as_str()).map(String::from),
            mine: p["owner"]["id"].as_str() == Some(uid.as_str()) || p["collaborative"].as_bool() == Some(true),
        }).filter(|p| !p.id.is_empty()).collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// รายชื่อเพลงของ "liked" (เพลงที่ถูกใจ), id เพลย์ลิสต์ หรือ ลิงก์ Spotify ของเพลย์ลิสต์/อัลบั้ม
#[tauri::command]
pub async fn spotify_tracks(app: AppHandle, src: String) -> Result<SpList, String> {
    tauri::async_runtime::spawn_blocking(move || {
        const MAX: usize = 2000;
        if src == "liked" {
            let items = pages(&app, &format!("{API}/me/tracks?limit=50"), MAX)?;
            return Ok(SpList { name: "Liked Songs".into(), tracks: items.iter().filter_map(|e| track(if e["track"].is_null() { &e["item"] } else { &e["track"] }, None)).collect() });
        }
        let (kind, id) = parse_link(&src).unwrap_or(("playlist", src.clone()));
        if !id.chars().all(|c| c.is_ascii_alphanumeric()) { return Err("ลิงก์ Spotify ไม่ถูกต้อง (รองรับลิงก์เพลย์ลิสต์และอัลบั้ม)".into()); }
        if kind == "album" {
            let a = get(&app, &format!("{API}/albums/{id}"))?;
            let name = a["name"].as_str().unwrap_or("Album").to_string();
            let mut items = a["tracks"]["items"].as_array().cloned().unwrap_or_default();
            if let Some(next) = a["tracks"]["next"].as_str().filter(|n| n.starts_with(API)) { items.extend(pages(&app, next, MAX)?); }
            return Ok(SpList { tracks: items.iter().filter_map(|t| track(t, Some(&name))).collect(), name });
        }
        let p = get(&app, &format!("{API}/playlists/{id}?fields=name"))?;
        let items = pages(&app, &format!("{API}/playlists/{id}/items?limit=100"), MAX)?;
        Ok(SpList {
            name: p["name"].as_str().unwrap_or("Spotify").to_string(),
            tracks: items.iter().filter_map(|e| track(if e["item"].is_null() { &e["track"] } else { &e["item"] }, None)).collect(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links() {
        assert_eq!(parse_link("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc"), Some(("playlist", "37i9dQZF1DXcBWIGoYBM5M".into())));
        assert_eq!(parse_link("https://open.spotify.com/intl-th/album/4aawyAB9vmqN3uQ7FjRGTy"), Some(("album", "4aawyAB9vmqN3uQ7FjRGTy".into())));
        assert_eq!(parse_link("spotify:album:4aawyAB9vmqN3uQ7FjRGTy"), Some(("album", "4aawyAB9vmqN3uQ7FjRGTy".into())));
        assert_eq!(parse_link("https://open.spotify.com/track/4aawyAB9vmqN3uQ7FjRGTy"), None);
        assert_eq!(parse_link("https://evil.com/playlist/37i9dQZF1DXcBWIGoYBM5M"), None);
    }
    #[test]
    fn pkce() { let c = b64(&Sha256::digest(b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")); assert_eq!(c, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"); }
}
