// หาเพลงบน YouTube ให้ตรงกับเพลงจาก Spotify (ชื่อ ศิลปิน ความยาว)
export type SpTrack = { title: string; artist: string; album: string; dur: number };
export type Hit = { title: string; uploader: string; dur: number; url: string; thumb?: string | null };

const norm = (s: string) => s.normalize('NFKC').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, ' ').trim();
// คำที่บอกว่าเป็นเวอร์ชันอื่น: ถ้าชื่อใน Spotify ไม่มีคำนี้ แต่ผลค้นหามี ถือว่าผิดเวอร์ชัน
const OTHER = ['live', 'cover', 'karaoke', 'instrumental', 'remix', 'sped up', 'speed up', 'slowed', 'nightcore', 'reverb', '8d', 'acoustic', 'piano', 'reaction', 'tutorial', 'แปล', 'คาราโอเกะ'];

export const query = (t: SpTrack) => (t.artist ? `${t.artist.split(',')[0].trim()} - ${t.title}` : t.title);

/** คะแนนความตรง: สูง = ตรง; ใช้คู่กับ pick() */
export function score(t: SpTrack, h: Hit): number {
  const ht = norm(h.title), up = norm(h.uploader), st = norm(t.title), a0 = norm(t.artist.split(',')[0]);
  let s = 0;
  const words = st.split(' ').filter((w) => w.length > 1 || /\p{Script=Thai}/u.test(w));
  const hitWords = words.filter((w) => ht.includes(w)).length;
  s += words.length ? (hitWords / words.length) * 4 : 0; // ชื่อเพลงตรงกี่ส่วน
  if (a0 && (ht.includes(a0) || up.includes(a0))) s += 2;
  if (/ topic$/.test(up)) s += 2; // ช่อง "ศิลปิน - Topic" = เสียงทางการจาก YouTube Music
  if (/official (audio|music video|video)/.test(ht)) s += 1;
  for (const w of OTHER) if (ht.includes(w) && !st.includes(w)) s -= 4;
  if (h.dur && t.dur) {
    const d = Math.abs(h.dur - t.dur);
    s += d <= 3 ? 4 : d <= 8 ? 1.5 : d <= 20 ? -1 : -6;
  } else s -= 1;
  return s;
}

/** เลือกผลที่ดีที่สุด: sure = มั่นใจพอให้โหลดเลย, alts = ตัวเลือกให้ผู้ใช้เลือกเอง */
export function pick(t: SpTrack, hits: Hit[]): { best: Hit | null; sure: boolean; alts: Hit[] } {
  const ranked = hits.map((h) => ({ h, s: score(t, h) })).sort((a, b) => b.s - a.s);
  const top = ranked[0];
  if (!top) return { best: null, sure: false, alts: [] };
  const dOk = !top.h.dur || !t.dur || Math.abs(top.h.dur - t.dur) <= 10;
  // รายชื่อที่วางเองไม่มีความยาวเพลงให้เทียบ จึงลดเกณฑ์ลงหนึ่งขั้น (ยังต้องชื่อ+ศิลปินตรง และไม่ใช่เวอร์ชันอื่น)
  return { best: top.h, sure: top.s >= (t.dur ? 7 : 6) && dOk, alts: ranked.slice(0, 4).map((x) => x.h) }; // เรียงจากตรงสุด ให้ผู้ใช้เลือกเองเมื่อไม่มั่นใจ
}

/** แปลงข้อความที่วาง (บรรทัดละเพลง "ศิลปิน - ชื่อเพลง") เป็นรายการเพลง; ไม่มีขีดก็ใช้ทั้งบรรทัดเป็นชื่อเพลง */
export function parseList(text: string): SpTrack[] {
  return text.split(/\r?\n/).map((l) => l.replace(/^\s*(\d+[.)]\s*|[-*•]\s+)/, '').trim()).filter((l) => l && !/^https?:/i.test(l)).slice(0, 500).map((l) => {
    const m = l.match(/^(.+?)\s+[-–—]\s+(.+)$/);
    return { title: (m ? m[2] : l).trim(), artist: (m ? m[1] : '').trim(), album: '', dur: 0 };
  });
}
