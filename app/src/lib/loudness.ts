// วัดความดังเฉลี่ยของเพลงเป็น LUFS (ตามแนว ITU-R BS.1770: K-weighting + gating) ไว้ปรับระดับเสียงให้แต่ละเพลงเท่ากัน

/** ความดังจากเสียงที่ผ่าน K-weighting แล้ว (ทุกช่องน้ำหนัก 1) หน่วย LUFS; เงียบสนิท/สั้นเกินไป = null */
export function lufsFromWeighted(chs: ArrayLike<number>[], sr: number): number | null {
  const n = chs[0]?.length ?? 0;
  const block = Math.round(sr * 0.4), hop = Math.round(sr * 0.1);
  if (n < block) return null;
  // ผลรวมกำลังสองแบบสะสม เพื่อหาค่าเฉลี่ยของแต่ละบล็อก 400 ms (ซ้อนทับ 75%) ได้ในเวลา O(n)
  const energies: number[] = [];
  const acc = chs.map((c) => {
    const a = new Float64Array(n + 1);
    for (let i = 0; i < n; i++) a[i + 1] = a[i] + c[i] * c[i];
    return a;
  });
  for (let s = 0; s + block <= n; s += hop) {
    let e = 0;
    for (const a of acc) e += (a[s + block] - a[s]) / block;
    energies.push(e);
  }
  const lk = (e: number) => -0.691 + 10 * Math.log10(e);
  const abs = energies.filter((e) => e > 0 && lk(e) > -70);
  if (!abs.length) return null;
  const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;
  const rel = lk(mean(abs)) - 10;
  const kept = abs.filter((e) => lk(e) > rel);
  return kept.length ? lk(mean(kept)) : null;
}

import { silenceEdges } from './fx';

const MAX_SEC = 20 * 60; // ยาวเกินนี้ข้าม (กินหน่วยความจำมาก)

/** ถอดรหัสไฟล์เสียง → K-weighting ด้วย OfflineAudioContext → LUFS (null = วัดไม่ได้) */
export async function measureLufs(bytes: ArrayBuffer): Promise<number | null> {
  const SR = 48000;
  const dec = await new OfflineAudioContext(2, 1, SR).decodeAudioData(bytes);
  if (dec.duration > MAX_SEC) return null;
  const ctx = new OfflineAudioContext(Math.min(2, dec.numberOfChannels), dec.length, SR);
  const src = ctx.createBufferSource();
  src.buffer = dec;
  const shelf = ctx.createBiquadFilter(); // high-shelf ~ +4 dB ที่ 1.5 kHz
  shelf.type = 'highshelf'; shelf.frequency.value = 1500; shelf.gain.value = 4;
  const hp = ctx.createBiquadFilter(); // high-pass ~ 38 Hz
  hp.type = 'highpass'; hp.frequency.value = 38; hp.Q.value = 0.5;
  src.connect(shelf).connect(hp).connect(ctx.destination);
  src.start();
  const out = await ctx.startRendering();
  return lufsFromWeighted(Array.from({ length: out.numberOfChannels }, (_, i) => out.getChannelData(i)), SR);
}

/** ตัวคูณระดับเสียง (0..1): ลดเพลงที่ดังกว่าเป้าหมายลง (ขยายเกิน 100% ไม่ได้ เพราะ audio.volume สูงสุด 1) */
export function normGain(lufs: number | null | undefined, target = -16): number {
  if (lufs == null || !isFinite(lufs)) return 1;
  return Math.min(1, Math.max(0.15, 10 ** ((target - lufs) / 20)));
}

/** ความเงียบช่วงต้น/ท้ายเพลง (วินาที) */
export async function detectSilence(bytes: ArrayBuffer): Promise<{ start: number; end: number } | null> {
  const dec = await new OfflineAudioContext(2, 1, 22050).decodeAudioData(bytes);
  if (dec.duration > MAX_SEC) return null;
  return silenceEdges(Array.from({ length: dec.numberOfChannels }, (_, i) => dec.getChannelData(i)), dec.sampleRate);
}

/** ประมาณจังหวะ (BPM) จากเสียงโมโน: ซองพลังงาน → ส่วนที่ดังขึ้น → autocorrelation ช่วง 70–180 BPM */
export function bpmFromSamples(x: ArrayLike<number>, sr: number): number | null {
  const hop = Math.round(sr / 100), win = hop * 2; // ~100 เฟรมต่อวินาที
  const n = Math.floor((x.length - win) / hop);
  if (n < 400) return null; // สั้นกว่า ~4 วินาที
  const env = new Float32Array(n);
  let prev = 0;
  for (let f = 0; f < n; f++) {
    let e = 0;
    for (let i = f * hop, end = i + win; i < end; i++) e += x[i] * x[i];
    e = Math.log1p(e * 1000);
    env[f] = Math.max(0, e - prev);
    prev = e;
  }
  const mean = env.reduce((a, b) => a + b, 0) / n;
  for (let f = 0; f < n; f++) env[f] -= mean;
  const fps = sr / hop;
  let best = 0, bestLag = 0;
  const minLag = Math.floor((60 / 180) * fps), maxLag = Math.ceil((60 / 70) * fps);
  for (let lag = minLag; lag <= maxLag; lag++) {
    let c = 0;
    for (let f = 0; f + lag < n; f++) c += env[f] * env[f + lag];
    c /= n - lag;
    if (c > best) { best = c; bestLag = lag; }
  }
  if (!bestLag || best <= 0) return null;
  // ปรับละเอียดด้วยการประมาณพาราโบลารอบจุดสูงสุด
  const ac = (lag: number) => { let c = 0; for (let f = 0; f + lag < n; f++) c += env[f] * env[f + lag]; return c / (n - lag); };
  const a = ac(bestLag - 1), b = best, c2 = ac(bestLag + 1);
  const shift = a - 2 * b + c2 ? (0.5 * (a - c2)) / (a - 2 * b + c2) : 0;
  return Math.round((60 * fps) / (bestLag + shift));
}

/** ถอดรหัสไฟล์แล้วหา BPM จากช่วงกลางเพลง 60 วินาที */
export async function detectBpm(bytes: ArrayBuffer): Promise<number | null> {
  const dec = await new OfflineAudioContext(1, 1, 11025).decodeAudioData(bytes);
  if (dec.duration > MAX_SEC) return null;
  const ch = dec.getChannelData(0), sr = dec.sampleRate;
  const mid = Math.floor(ch.length / 2), half = Math.min(ch.length / 2, 30 * sr);
  return bpmFromSamples(ch.subarray(Math.max(0, mid - half), Math.min(ch.length, mid + half)), sr);
}

/** จุดสูงสุดของเสียง n ช่วง (0–9 ต่อช่วง) ไว้วาดรูปคลื่นบนแถบเลื่อนเวลา */
export function peaksString(x: ArrayLike<number>, n = 120): string {
  const step = Math.max(1, Math.floor(x.length / n));
  const peaks: number[] = [];
  for (let k = 0; k < n; k++) {
    let m = 0;
    for (let i = k * step, end = Math.min(x.length, i + step); i < end; i += 4) m = Math.max(m, Math.abs(x[i]));
    peaks.push(m);
  }
  const top = Math.max(...peaks, 1e-6);
  return peaks.map((p) => Math.min(9, Math.round((p / top) * 9))).join('');
}

/** ถอดรหัสครั้งเดียว ได้ทั้ง BPM และรูปคลื่น */
export async function bpmAndWave(bytes: ArrayBuffer): Promise<{ bpm: number | null; wave: string }> {
  const dec = await new OfflineAudioContext(1, 1, 11025).decodeAudioData(bytes);
  if (dec.duration > MAX_SEC) return { bpm: null, wave: '' };
  const ch = dec.getChannelData(0), sr = dec.sampleRate;
  const mid = Math.floor(ch.length / 2), half = Math.min(ch.length / 2, 30 * sr);
  return { bpm: bpmFromSamples(ch.subarray(Math.max(0, mid - half), Math.min(ch.length, mid + half)), sr), wave: peaksString(ch) };
}
