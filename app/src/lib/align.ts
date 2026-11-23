// จับคู่เนื้อเพลงที่ผู้ใช้มี กับคำที่ AI (whisper) ได้ยินพร้อมเวลา → เวลาเริ่มของแต่ละบรรทัด
// วิธี: เทียบระดับตัวอักษร (ตัดช่องว่าง/เครื่องหมาย) ด้วย global alignment แล้วใช้เวลาของตัวอักษรตัวแรกที่จับคู่ได้ในบรรทัด

/** ms = เวลาเริ่ม, end = เวลาจบ (ถ้ามี และข้อความยาวกว่า 1 ตัว จะกระจายเวลาตามตัวอักษร) */
export type Word = { ms: number; text: string; end?: number };

const keep = (s: string) => s.normalize('NFKD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '');

/** คืนเวลาเริ่ม (วินาที) ของแต่ละบรรทัด; บรรทัดที่หาไม่เจอเป็น null */
export function alignLines(lines: string[], words: Word[]): (number | null)[] {
  return alignDetail(lines, words).times;
}

/** เหมือน alignLines แต่คืนค่าความมั่นใจ (0..1 = สัดส่วนตัวอักษรของบรรทัดที่ฟังตรงกับเสียง) ด้วย เอาไว้เตือนบรรทัดที่ควรตรวจ */
export function alignDetail(lines: string[], words: Word[]): { times: (number | null)[]; conf: number[] } {
  // ตัวอักษรเนื้อเพลง + บรรทัดของแต่ละตัว
  const L: string[] = [], lineOf: number[] = [];
  lines.forEach((ln, i) => { for (const c of keep(ln)) { L.push(c); lineOf.push(i); } });
  // ตัวอักษรที่ได้ยิน + เวลาของแต่ละตัว (กระจายเวลาภายในคำ/โทเค็นไม่ได้ จึงใช้เวลาเริ่มของคำ)
  const R: string[] = [], tOf: number[] = [];
  for (const w of words) {
    const cs = [...keep(w.text)];
    // ช่วงยาว (โหมดสำรองที่ได้เป็นประโยค): กระจายเวลาตามตำแหน่งตัวอักษร ไม่ให้ทุกตัวได้เวลาเริ่มเดียวกัน
    const span = w.end !== undefined && cs.length > 3 ? Math.max(0, w.end - w.ms) / 1000 : 0;
    cs.forEach((c, k) => { R.push(c); tOf.push(w.ms / 1000 + (span * k) / cs.length); });
  }
  const n = L.length, m = R.length;
  const out: (number | null)[] = lines.map(() => null);
  const conf: number[] = lines.map(() => 0);
  if (!n || !m) return { times: out, conf };
  // Needleman–Wunsch แบบไม่คิดค่าช่องว่างที่ปลาย (เพลงมีอินโทร/ท่อนที่ AI ฟังผิดได้)
  const MATCH = 2, MIS = -1, GAP = -1;
  const W = m + 1;
  const S = new Int32Array((n + 1) * W);
  const B = new Uint8Array((n + 1) * W); // 1 = ทแยง, 2 = ขึ้น (ข้ามตัวในเนื้อ), 3 = ซ้าย (ข้ามตัวที่ได้ยิน)
  for (let i = 1; i <= n; i++) { S[i * W] = i * GAP; B[i * W] = 2; }
  for (let j = 1; j <= m; j++) { S[j] = 0; B[j] = 3; } // ข้ามเสียงช่วงต้นได้ฟรี
  for (let i = 1; i <= n; i++) {
    const li = L[i - 1];
    for (let j = 1; j <= m; j++) {
      const d = S[(i - 1) * W + j - 1] + (li === R[j - 1] ? MATCH : MIS);
      const u = S[(i - 1) * W + j] + GAP;
      const l = S[i * W + j - 1] + (i === n ? 0 : GAP); // ข้ามเสียงช่วงท้ายได้ฟรี
      if (d >= u && d >= l) { S[i * W + j] = d; B[i * W + j] = 1; }
      else if (u >= l) { S[i * W + j] = u; B[i * W + j] = 2; }
      else { S[i * W + j] = l; B[i * W + j] = 3; }
    }
  }
  // ย้อนรอย: ตัวอักษรเนื้อเพลง i ↔ ตัวที่ได้ยิน j (เฉพาะที่ตรงกันจริง)
  const match = new Int32Array(n).fill(-1);
  for (let i = n, j = m; i > 0 || j > 0; ) {
    const b = B[i * W + j];
    if (i > 0 && j > 0 && b === 1) { if (L[i - 1] === R[j - 1]) match[i - 1] = j - 1; i--; j--; }
    else if (i > 0 && (b === 2 || j === 0)) i--;
    else j--;
  }
  // เวลาของบรรทัด = ใช้เวลาจากตัวที่จับคู่ได้ช่วงต้นบรรทัด โดยต้องจับคู่ได้อย่างน้อย 2 ตัวใน 6 ตัวแรก (กันจับคู่บังเอิญ)
  const firstIdx = new Map<number, number[]>();
  L.forEach((_, k) => { const ln = lineOf[k]; if (!firstIdx.has(ln)) firstIdx.set(ln, []); firstIdx.get(ln)!.push(k); });
  for (const [ln, ks] of firstIdx) {
    conf[ln] = ks.filter((k) => match[k] >= 0).length / ks.length;
    const head = ks.slice(0, 6).filter((k) => match[k] >= 0);
    if (head.length >= Math.min(2, ks.length)) {
      const k = head[0];
      out[ln] = tOf[match[k]];
    } else {
      // ใช้ตัวใดก็ได้ในบรรทัดที่จับคู่ได้ ถ้าจับได้เกินครึ่งบรรทัด
      const hit = ks.filter((k) => match[k] >= 0);
      if (hit.length >= ks.length / 2) out[ln] = tOf[match[hit[0]]];
    }
  }
  // เวลาต้องเพิ่มขึ้นเรื่อยๆ: ตัดค่าที่ย้อนหลังทิ้ง แล้วเติมช่องที่ขาดด้วยการประมาณเชิงเส้น
  let last = -1;
  for (let i = 0; i < out.length; i++) { const v = out[i]; if (v !== null) { if (v < last) { out[i] = null; conf[i] = 0; } else last = v; } }
  return { times: out, conf };
}

/** เติมบรรทัดที่หาเวลาไม่เจอโดยแบ่งช่วงระหว่างบรรทัดก่อน/หลังเท่าๆ กัน */
export function fillGaps(times: (number | null)[], dur: number): number[] {
  const out = times.slice();
  const idx = out.map((v, i) => (v !== null ? i : -1)).filter((i) => i >= 0);
  if (!idx.length) return out.map((_, i) => (dur / (out.length + 1)) * (i + 1));
  for (let i = 0; i < out.length; i++) {
    if (out[i] !== null) continue;
    const a = [...idx].reverse().find((k) => k < i), b = idx.find((k) => k > i);
    const ta = a !== undefined ? out[a]! : 0, tb = b !== undefined ? out[b]! : Math.max(ta + 2, dur);
    const ia = a ?? -1, ib = b ?? out.length;
    out[i] = ta + ((tb - ta) * (i - ia)) / (ib - ia);
  }
  return out as number[];
}

/** บรรทัดที่ควรให้ผู้ใช้ตรวจ (เลข 0-based): หาเวลาไม่เจอ / ฟังตรงน้อยกว่า 50% / เวลาชนกับบรรทัดก่อน / ห่างเกิน 40 วินาที */
export function weakLines(times: (number | null)[], conf: number[], full: number[]): number[] {
  const bad = new Set<number>();
  times.forEach((t, i) => { if (t === null || conf[i] < 0.5) bad.add(i); });
  for (let i = 1; i < full.length; i++) {
    const g = full[i] - full[i - 1];
    if (g < 0.4 || g > 40) bad.add(i);
  }
  return [...bad].sort((a, b) => a - b);
}
