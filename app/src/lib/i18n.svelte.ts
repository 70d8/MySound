import EN, { EN_RX } from './en';

// ภาษาของแอป: ใช้ข้อความไทยเป็น key (ไทย = ข้อความต้นฉบับ, อังกฤษ = ตารางแปลใน en.ts)
// ตัวแปรแทนค่าใช้ {0} {1} ... ทั้งในข้อความไทยและอังกฤษ
export type Lang = 'en' | 'th';
export const i18n = $state<{ lang: Lang }>({ lang: 'en' });

export function tr(th: string, ...a: unknown[]): string {
  let s = th;
  if (i18n.lang === 'en') {
    s = EN[th];
    if (s === undefined) {
      const r = EN_RX.find(([re]) => re.test(th));
      s = r ? th.replace(r[0], r[1]) : th;
    }
  }
  return a.length ? s.replace(/\{(\d)\}/g, (_, i) => String(a[+i])) : s;
}
