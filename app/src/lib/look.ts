// หน้าตาของแอป: ชุดสี + เลย์เอาต์ที่ปรับได้ ทุกอย่างแปลงเป็นตัวแปร CSS ที่ :root
export type Look = {
  accent: string;
  base: string; // พื้นแผง
  raised: string; // พื้นการ์ด/ช่องกรอก
  outer: string; // พื้นหลังช่องว่างระหว่างแผง
  sidebar: number; // px
  right: number; // px
  radius: number; // px
  row: number; // px ความสูงแถวเพลง
  font: number; // % ขนาดตัวอักษร
  tint: boolean; // ไล่สีแผงขวาตามปกเพลง
  startPanel: boolean; // เปิดแผงคิวตอนเริ่มแอป
  viz: boolean; // ตัวแสดงคลื่นเสียง
  bg: string; // พาธรูปพื้นหลัง (ไฟล์ในโฟลเดอร์เพลงของแอป) ว่าง = ไม่ใช้
  bgAlpha: number; // % ความทึบของแผงเมื่อมีรูปพื้นหลัง (ต่ำ = เห็นรูปมาก)
  bgBlur: number; // px เบลอรูปพื้นหลัง
};

export const THEMES: { name: string; accent: string; base: string; raised: string; outer: string }[] = [
  { name: 'เทอร์ควอยซ์', accent: '#5eead4', base: '#0f1115', raised: '#181b21', outer: '#000000' },
  { name: 'มิดไนท์', accent: '#7aa2ff', base: '#0d1321', raised: '#162036', outer: '#060a14' },
  { name: 'กุหลาบ', accent: '#ff7aa8', base: '#1a1015', raised: '#26171e', outer: '#0d0709' },
  { name: 'อำพัน', accent: '#fbbf24', base: '#17130d', raised: '#231c12', outer: '#0c0905' },
  { name: 'ป่าเขียว', accent: '#86efac', base: '#0e1511', raised: '#16221b', outer: '#060b08' },
  { name: 'สว่าง', accent: '#0d9488', base: '#f6f7f9', raised: '#ffffff', outer: '#e1e5ea' }
];

export const LAYOUTS: { name: string; hint: string; v: Pick<Look, 'sidebar' | 'right' | 'radius' | 'row' | 'font'> }[] = [
  { name: 'คลาสสิก', hint: 'สมดุล เหมือนค่าเริ่มต้น', v: { sidebar: 240, right: 320, radius: 8, row: 56, font: 100 } },
  { name: 'กะทัดรัด', hint: 'เห็นเพลงต่อหน้าจอมากขึ้น', v: { sidebar: 200, right: 280, radius: 6, row: 44, font: 94 } },
  { name: 'สบายตา', hint: 'ใหญ่ โปร่ง มุมโค้งมาก', v: { sidebar: 280, right: 360, radius: 16, row: 68, font: 108 } }
];

export const DEFAULT_LOOK: Look = {
  ...THEMES[0],
  ...LAYOUTS[0].v,
  tint: true,
  startPanel: true,
  viz: true,
  bg: '',
  bgAlpha: 60,
  bgBlur: 0
};

const hex = (s: unknown, d: string) => (typeof s === 'string' && /^#[0-9a-f]{6}$/i.test(s) ? s : d);
const num = (n: unknown, lo: number, hi: number, d: number) => (typeof n === 'number' && isFinite(n) ? Math.min(hi, Math.max(lo, n)) : d);

/** รับค่าจากไฟล์ที่อาจเก่า/พัง แล้วคืนค่าที่ใช้ได้เสมอ */
export function normalizeLook(x: Partial<Look> | null | undefined): Look {
  const d = DEFAULT_LOOK;
  const o = x ?? {};
  return {
    accent: hex(o.accent, d.accent),
    base: hex(o.base, d.base),
    raised: hex(o.raised, d.raised),
    outer: hex(o.outer, d.outer),
    sidebar: num(o.sidebar, 180, 320, d.sidebar),
    right: num(o.right, 240, 440, d.right),
    radius: num(o.radius, 0, 20, d.radius),
    row: num(o.row, 40, 80, d.row),
    font: num(o.font, 85, 125, d.font),
    tint: o.tint ?? d.tint,
    startPanel: o.startPanel ?? d.startPanel,
    viz: o.viz ?? d.viz,
    bg: typeof o.bg === 'string' ? o.bg : '',
    bgAlpha: num(o.bgAlpha, 15, 100, d.bgAlpha),
    bgBlur: num(o.bgBlur, 0, 24, d.bgBlur)
  };
}

/** ความสว่างสัมพัทธ์ 0..1 ของสี #rrggbb */
export function lum(h: string): number {
  const c = [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
export const isLight = (l: Look) => lum(l.base) > 0.4;

/** ตัวแปร CSS ทั้งหมดที่ได้จากค่าที่ผู้ใช้เลือก (ตัวอักษร/เส้น/พื้นตอนชี้ คำนวณให้เอง) */
export function lookVars(l: Look): Record<string, string> {
  const light = isLight(l);
  const text = light ? '#14171c' : '#f2f4f7';
  return {
    '--bg-base': l.base,
    '--panel-bg': l.bg ? `color-mix(in srgb, ${l.base} ${l.bgAlpha}%, transparent)` : l.base,
    '--bg-raised': l.raised,
    '--bg-hover': `color-mix(in srgb, ${l.raised} 86%, ${text})`,
    '--bg-outer': l.outer,
    '--text': text,
    '--text-dim': light ? '#5b6472' : '#9aa3af',
    '--accent': l.accent,
    '--on-accent': lum(l.accent) > 0.35 ? '#04201c' : '#ffffff',
    '--line': `color-mix(in srgb, ${text} 8%, transparent)`,
    '--line2': `color-mix(in srgb, ${text} 13%, transparent)`,
    '--line3': `color-mix(in srgb, ${text} 18%, transparent)`,
    '--danger': light ? '#dc2626' : '#f87171',
    '--radius-panel': `${l.radius}px`,
    '--radius-card': `${Math.max(2, Math.round(l.radius * 0.75))}px`,
    '--sidebar-w': `${l.sidebar}px`,
    '--right-w': `${l.right}px`,
    '--row-h': `${l.row}px`
  };
}
