// ค้นหาในคลัง: ข้อความธรรมดา (ทนพิมพ์ผิดเล็กน้อย, ไทยไม่เว้นวรรคก็เจอ) + ตัวกรองแบบ key:value
// ตัวอย่าง: skeleton rating:>=4 added:<7d tag:chill plays:>3 dur:>5m fav:yes artist:mandy album:"angel rage"
import type { Track } from './data';

export type Cond = { key: string; op: string; val: string };
export type Query = { words: string[]; conds: Cond[] };

const KEYS = ['artist', 'album', 'title', 'tag', 'rating', 'plays', 'added', 'last', 'dur', 'fav', 'note', 'skips', 'bpm'];

export function norm(s: string): string {
  return s.normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase();
}

export function parseQuery(q: string): Query {
  const words: string[] = [], conds: Cond[] = [];
  const re = /(\w+):(>=|<=|>|<|=)?("([^"]*)"|\S+)|"([^"]*)"|(\S+)/g;
  for (let m; (m = re.exec(q)); ) {
    if (m[1] && KEYS.includes(m[1].toLowerCase())) conds.push({ key: m[1].toLowerCase(), op: m[2] ?? '', val: m[4] ?? m[3] });
    else { const w = m[5] ?? m[6] ?? m[0]; if (w.trim()) words.push(norm(w)); }
  }
  return { words, conds };
}

function cmp(a: number, op: string, b: number): boolean {
  return op === '>' ? a > b : op === '<' ? a < b : op === '>=' ? a >= b : op === '<=' ? a <= b : a === b;
}
/** "7d" "2w" "3m"(เดือน) "1y" → จำนวนวัน; "2026-11-01" → วันที่ */
function daysAgo(iso: string): number {
  return iso ? (Date.now() - new Date(iso).getTime()) / 864e5 : Infinity;
}
function parseSpan(v: string): number | null {
  const m = /^(\d+(?:\.\d+)?)([dwmy])$/i.exec(v);
  if (!m) return null;
  return +m[1] * { d: 1, w: 7, m: 30, y: 365 }[m[2].toLowerCase() as 'd'];
}
function parseDur(v: string): number {
  const m = /^(\d+(?:\.\d+)?)(s|m|h)?$/i.exec(v);
  if (m) return +m[1] * ({ s: 1, m: 60, h: 3600 }[(m[2] ?? 's').toLowerCase() as 's']);
  const mm = /^(\d+):(\d{2})$/.exec(v);
  return mm ? +mm[1] * 60 + +mm[2] : NaN;
}

function condOk(t: Track, c: Cond, artistOf: (t: Track) => string): boolean {
  const v = norm(c.val);
  switch (c.key) {
    case 'artist': return norm(artistOf(t)).includes(v);
    case 'album': return norm(t.album ?? '').includes(v);
    case 'title': return norm(t.title).includes(v);
    case 'note': return norm(t.note ?? '').includes(v);
    case 'tag': return (t.tags ?? []).some((x) => norm(x) === v || norm(x).startsWith(v));
    case 'fav': return !!t.fav === /^(y|yes|1|true|ใช่)$/i.test(c.val);
    case 'rating': return cmp(t.rating ?? 0, c.op || '=', +c.val);
    case 'plays': return cmp(t.plays ?? 0, c.op || '=', +c.val);
    case 'skips': return cmp(t.skips ?? 0, c.op || '=', +c.val);
    case 'bpm': return t.bpm != null && cmp(t.bpm, c.op || '=', +c.val);
    case 'dur': { const d = parseDur(c.val); return !isNaN(d) && cmp(t.dur, c.op || '>=', d); }
    case 'added': case 'last': {
      const iso = c.key === 'added' ? t.added : t.last;
      const span = parseSpan(c.val);
      // added:<7d = เพิ่มภายใน 7 วัน, added:>30d = เก่ากว่า 30 วัน
      if (span !== null) return cmp(daysAgo(iso), c.op || '<', span);
      return iso ? cmp(iso.localeCompare(c.val), c.op || '=', 0) : false;
    }
  }
  return true;
}

function lev1(a: string, b: string): boolean {
  // ระยะแก้คำ ≤ 1 (พิมพ์ผิด/ขาด/เกินหนึ่งตัว หรือสลับตัวติดกัน)
  if (Math.abs(a.length - b.length) > 1) return false;
  let i = 0, j = 0, e = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) { i++; j++; continue; }
    if (++e > 1) return false;
    if (a[i + 1] === b[j] && a[i] === b[j + 1] && a.length === b.length) { i += 2; j += 2; continue; }
    if (a.length > b.length) i++; else if (b.length > a.length) j++; else { i++; j++; }
  }
  return e + (a.length - i) + (b.length - j) <= 1;
}

export function hay(t: Track, artistOf: (t: Track) => string): string {
  return norm(`${t.title} ${artistOf(t)} ${t.album ?? ''} ${(t.tags ?? []).join(' ')}`);
}

export function wordOk(w: string, h: string): boolean {
  if (h.includes(w)) return true;
  const hs = h.replace(/\s+/g, '');
  if (hs.includes(w.replace(/\s+/g, ''))) return true;
  if (w.length < 4) return false;
  // พิมพ์ผิดหนึ่งตัว เทียบกับทุกช่วงของคำ (เช่น "skeletn" เจอใน "wifiskeleton")
  return h.split(/\s+/).some((x) => {
    for (let L = w.length - 1; L <= w.length + 1; L++) for (let i = 0; i + L <= x.length; i++) if (lev1(w, x.slice(i, i + L))) return true;
    return false;
  });
}

export function matches(t: Track, q: Query, artistOf: (t: Track) => string = (t) => t.artist): boolean {
  if (!q.conds.every((c) => condOk(t, c, artistOf))) return false;
  if (!q.words.length) return true;
  const h = hay(t, artistOf);
  return q.words.every((w) => wordOk(w, h));
}

/** กลุ่มเพลงที่น่าจะซ้ำกัน: ชื่อ+ศิลปินเหมือนกันหลังตัดวงเล็บ/คำว่า official ฯลฯ และความยาวต่างกันไม่เกิน 3 วินาที */
export function dupKey(t: Track): string {
  const clean = (s: string) => norm(s).replace(/[([{].*?[)\]}]/g, '').replace(/\b(official|video|audio|lyrics?|mv|hd|hq|music)\b/g, '').replace(/[^\p{L}\p{N}]+/gu, '');
  return clean(t.artist) + '|' + clean(t.title);
}
export function findDuplicates(lib: Track[]): Track[][] {
  const m = new Map<string, Track[]>();
  for (const t of lib) { const k = dupKey(t); m.set(k, [...(m.get(k) ?? []), t]); }
  const out: Track[][] = [];
  for (const g of m.values()) {
    if (g.length < 2) continue;
    const sorted = [...g].sort((a, b) => a.dur - b.dur);
    let cur = [sorted[0]];
    for (const t of sorted.slice(1)) {
      if (Math.abs(t.dur - cur[cur.length - 1].dur) <= 3) cur.push(t);
      else { if (cur.length > 1) out.push(cur); cur = [t]; }
    }
    if (cur.length > 1) out.push(cur);
  }
  return out;
}
