//! แปลผลลัพธ์บรรทัดต่อบรรทัดของ yt-dlp (ส่วนนี้ไม่แตะ I/O จึงเทสต์ได้ง่าย)

/// ตัวนำหน้าบรรทัดที่เรากำหนดเองผ่าน --print / --progress-template
pub const P_PROGRESS: &str = "MS_PROGRESS";
pub const P_TITLE: &str = "MS_TITLE";
pub const P_DONE: &str = "MS_DONE";

#[derive(Debug, PartialEq)]
pub enum Line {
    Progress(f32),
    Title(String),
    Done(Done),
}

#[derive(Debug, PartialEq, Clone, serde::Serialize)]
pub struct Done {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub dur: u32,
    pub playlist: Option<String>,
    pub url: String,
    pub thumb: Option<String>,
    pub path: String,
    pub cover: Option<String>,
}

fn field(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty() && s != "NA").then(|| s.to_string())
}

pub fn parse(line: &str) -> Option<Line> {
    let line = line.trim_end_matches(['\r', '\n']);
    if let Some(rest) = line.strip_prefix(P_PROGRESS) {
        let pct = rest.trim().trim_end_matches('%').trim().parse::<f32>().ok()?;
        return Some(Line::Progress(pct.clamp(0.0, 100.0)));
    }
    if let Some(rest) = line.strip_prefix(P_TITLE) {
        return Some(Line::Title(field(rest.trim_start_matches('\t'))?));
    }
    if let Some(rest) = line.strip_prefix(P_DONE) {
        // MS_DONE \t title \t uploader \t album \t duration \t playlist \t url \t thumbnail \t filepath
        let f: Vec<&str> = rest.trim_start_matches('\t').splitn(8, '\t').collect();
        if f.len() < 8 {
            return None;
        }
        return Some(Line::Done(Done {
            title: field(f[0]).unwrap_or_else(|| "ไม่มีชื่อ".into()),
            artist: field(f[1]).unwrap_or_else(|| "ไม่ทราบศิลปิน".into()),
            album: field(f[2]).unwrap_or_else(|| "ดาวน์โหลด".into()),
            dur: f[3].trim().parse::<f64>().map(|d| d.round() as u32).unwrap_or(0),
            playlist: field(f[4]),
            url: field(f[5]).unwrap_or_default(),
            thumb: field(f[6]).filter(|u| u.starts_with("http")),
            path: field(f[7])?,
            cover: None,
        }));
    }
    None
}


/// ทำชื่อวิดีโอ YouTube ให้เป็นคำค้นที่ Genius เข้าใจ (ตัดวงเล็บ, official video, lyrics ฯลฯ)
pub fn clean_query(title: &str, uploader: &str) -> String {
    const NOISE: [&str; 12] = ["official", "video", "audio", "lyrics", "lyric", "mv", "m/v", "hd", "hq", "4k", "visualizer", "music"];
    let mut depth = 0;
    let no_brackets: String = title
        .chars()
        .filter(|c| match c {
            '(' | '[' | '【' | '（' => { depth += 1; false }
            ')' | ']' | '】' | '）' => { depth = (depth - 1).max(0); false }
            _ => depth == 0,
        })
        .collect();
    let head = no_brackets.split(" | ").next().unwrap_or("");
    let words: Vec<&str> = head.split_whitespace().filter(|w| !NOISE.contains(&w.to_lowercase().as_str())).collect();
    let t = words.join(" ");
    let up = uploader.trim().trim_end_matches(" - Topic").trim_end_matches("VEVO").trim();
    if t.contains(" - ") || up.is_empty() || up == "NA" { t } else { format!("{up} {t}") }
}

/// หยิบ URL ปกจากผลค้นหา Genius (/api/search/song) ผลแรกที่มีปก
pub fn pick_cover(v: &serde_json::Value) -> Option<String> {
    v["response"]["sections"].as_array()?.iter().flat_map(|s| s["hits"].as_array().into_iter().flatten()).find_map(|h| {
        let r = &h["result"];
        let u = r["song_art_image_url"].as_str().or(r["header_image_url"].as_str())?;
        (!u.contains("default_cover_image")).then(|| u.to_string())
    })
}


#[derive(Debug, PartialEq, serde::Serialize)]
pub struct Hit {
    pub title: String,
    pub uploader: String,
    pub dur: u32,
    pub url: String,
    pub thumb: Option<String>,
}

/// แปลงผลค้นหา (yt-dlp --flat-playlist -j) 1 บรรทัดเป็นรายการ; ข้ามถ้าไม่มีชื่อหรือลิงก์
pub fn parse_hit(v: &serde_json::Value) -> Option<Hit> {
    let s = |k: &str| v[k].as_str().filter(|x| !x.trim().is_empty());
    let url = s("webpage_url").or(s("url")).filter(|u| u.starts_with("http"))?;
    let thumb = s("thumbnail").or_else(|| v["thumbnails"].as_array()?.iter().rev().find_map(|t| t["url"].as_str())).map(String::from);
    Some(Hit {
        title: s("title")?.to_string(),
        uploader: s("channel").or(s("uploader")).unwrap_or("").to_string(),
        dur: v["duration"].as_f64().map_or(0, |d| d.round() as u32),
        url: url.to_string(),
        thumb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress() {
        assert_eq!(parse("MS_PROGRESS  42.3%"), Some(Line::Progress(42.3)));
        assert_eq!(parse("MS_PROGRESS 100%\r\n"), Some(Line::Progress(100.0)));
        assert_eq!(parse("MS_PROGRESS Unknown"), None);
    }

    #[test]
    fn title() {
        assert_eq!(parse("MS_TITLE\tเพลง A"), Some(Line::Title("เพลง A".into())));
        assert_eq!(parse("MS_TITLE\tNA"), None);
    }

    #[test]
    fn done_with_missing_fields() {
        let l = parse("MS_DONE\tSong\tNA\tNA\t213.6\tNA\thttps://x/y\thttps://t/i.jpg\tC:\\a\\b.m4a").unwrap();
        assert_eq!(
            l,
            Line::Done(Done {
                title: "Song".into(),
                artist: "ไม่ทราบศิลปิน".into(),
                album: "ดาวน์โหลด".into(),
                dur: 214,
                playlist: None,
                url: "https://x/y".into(),
                thumb: Some("https://t/i.jpg".into()),
                path: "C:\\a\\b.m4a".into(),
                cover: None
            })
        );
    }

    #[test]
    fn ignores_noise() {
        assert_eq!(parse("[youtube] Extracting URL"), None);
        assert_eq!(parse("MS_DONE\tonly\tthree"), None);
        assert_eq!(parse("MS_DONE\tT\tA\tB\t10\tNA\tu\tNA\tNA"), None); // ไม่มีไฟล์ = ล้มเหลว
    }

    #[test]
    fn query_cleaning() {
        assert_eq!(clean_query("Artist - Song (Official Music Video) [HD]", "ArtistVEVO"), "Artist - Song");
        assert_eq!(clean_query("Song Name | Live", "Cool Band - Topic"), "Cool Band Song Name");
        assert_eq!(clean_query("Lyrics", "NA"), "");
    }

    #[test]
    fn cover_pick() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"response":{"sections":[{"hits":[{"result":{"song_art_image_url":"https://images.genius.com/default_cover_image.png"}},{"result":{"song_art_image_url":"https://images.genius.com/a.jpg"}}]}]}}"#,
        ).unwrap();
        assert_eq!(pick_cover(&v).as_deref(), Some("https://images.genius.com/a.jpg"));
        assert_eq!(pick_cover(&serde_json::json!({})), None);
    }

    #[test]
    fn search_hit() {
        let v = serde_json::json!({"title":"Song","channel":"Chan","duration":201.4,"url":"https://www.youtube.com/watch?v=abc","thumbnails":[{"url":"https://i/a.jpg"},{"url":"https://i/b.jpg"}]});
        let h = parse_hit(&v).unwrap();
        assert_eq!((h.dur, h.thumb.as_deref(), h.uploader.as_str()), (201, Some("https://i/b.jpg"), "Chan"));
        assert_eq!(parse_hit(&serde_json::json!({"title":"x","url":"abc"})), None); // url ไม่ใช่ลิงก์เต็ม
        assert_eq!(parse_hit(&serde_json::json!({"url":"https://x"})), None); // ไม่มีชื่อ
    }
}
