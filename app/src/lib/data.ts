export type Track = {
  id: number;
  title: string;
  artist: string;
  album: string;
  dur: number; // วินาที
  src: 'youtube' | 'soundcloud';
  added: string; // วันที่เพิ่ม
  last: string; // วันที่ฟังล่าสุด
  plays: number;
  path: string; // ไฟล์เสียงบนเครื่อง
  cover?: string | null; // ไฟล์ปกบนเครื่อง
  url?: string; // ลิงก์ต้นทาง ใช้กันโหลดซ้ำ
  fav?: boolean; // ถูกใจ
  rating?: number; // 0-5 ดาว
  lrc?: string; // เนื้อเพลงที่ผู้ใช้ใส่เอง (LRC หรือข้อความธรรมดา)
  coverUrl?: string | null; // ลิงก์รูปปกบนเน็ต ไว้ส่งให้ Discord (ไฟล์ในเครื่อง Discord อ่านไม่ได้)
};

export type Playlist = { id: number; name: string; ids: number[] };

import { convertFileSrc } from '@tauri-apps/api/core';

export const fmt = (s: number) =>
  `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`;

const dateFmt = { th: new Intl.DateTimeFormat('th-TH', { day: 'numeric', month: 'short', year: '2-digit' }), en: new Intl.DateTimeFormat('en-GB', { day: 'numeric', month: 'short', year: '2-digit' }) };
export const fmtDate = (iso: string, lang: 'th' | 'en' = 'en') => dateFmt[lang].format(new Date(iso));

// สีปกจากชื่อเพลง (ยังไม่ดึงปกจริง)
export const hue = (s: string) => {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) % 360;
  return h;
};
export const cover = (s: string) =>
  `linear-gradient(135deg, hsl(${hue(s)} 55% 46%), hsl(${(hue(s) + 55) % 360} 60% 28%))`;

// พื้นหลังปกของเพลง: ใช้รูปจริงถ้ามี ไม่งั้นใช้ไล่สีจากชื่อ
export const art = (t: { title: string; cover?: string | null }) =>
  t.cover ? `center / cover no-repeat url("${convertFileSrc(t.cover)}")` : cover(t.title);

// ---------- เนื้อเพลง LRC ----------
export type LrcLine = { t: number; text: string };
/** "[01:23.45] ข้อความ" -> {t: 83.45}. รองรับหลาย timestamp ต่อบรรทัด และข้าม tag [ar:..] */
export function parseLrc(src: string): LrcLine[] {
  const out: LrcLine[] = [];
  for (const raw of src.split(/\r?\n/)) {
    const stamps = [...raw.matchAll(/\[(\d+):(\d+(?:\.\d+)?)\]/g)];
    if (!stamps.length) continue;
    const text = raw.replace(/\[[^\]]*\]/g, '').trim();
    for (const m of stamps) out.push({ t: +m[1] * 60 + +m[2], text });
  }
  return out.sort((a, b) => a.t - b.t);
}
/** ดัชนีบรรทัดที่กำลังร้อง ณ เวลา pos (-1 = ยังไม่ถึงบรรทัดแรก) */
export function lrcIndex(lines: LrcLine[], pos: number): number {
  let lo = 0, hi = lines.length - 1, ans = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (lines[mid].t <= pos) (ans = mid), (lo = mid + 1);
    else hi = mid - 1;
  }
  return ans;
}

// ---------- ลิงก์ ----------
/** คีย์เทียบลิงก์ซ้ำ: YouTube ใช้ video id, SoundCloud ใช้พาธ */
export function linkKey(u: string): string {
  const y = u.match(/(?:youtu\.be\/|[?&]v=|\/shorts\/)([\w-]{11})/);
  if (y) return 'yt:' + y[1];
  const sc = u.match(/soundcloud\.com\/([^?#]+)/);
  return sc ? 'sc:' + sc[1].replace(/\/$/, '').toLowerCase() : u.trim();
}
/** ลิงก์ playlist ของ YouTube (list=...) หรือ SoundCloud (/sets/) */
export const isPlaylistLink = (u: string) => /youtube\.com\/playlist\?|soundcloud\.com\/[^/]+\/sets\//.test(u);
export const isSupportedLink = (u: string) => /youtu\.?be|soundcloud\.com/.test(u);

/** อายุเวอร์ชัน yt-dlp (วัน) จากเลขเวอร์ชันแบบ 2026.11.01 */
export function versionAgeDays(v: string, now = new Date()): number | null {
  const m = v.match(/^(\d{4})\.(\d{2})\.(\d{2})/);
  return m ? Math.floor((+now - Date.UTC(+m[1], +m[2] - 1, +m[3])) / 864e5) : null;
}
