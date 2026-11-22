// เอฟเฟกต์เสียง (Web Audio): อีควอไลเซอร์ 10 แบนด์, บาลานซ์, โมโน, ครอสเฟด/เล่นต่อเนื่อง (ใช้ <audio> 2 ตัวสลับกัน)
// สร้างกราฟเมื่อจำเป็นเท่านั้น (ค่าเริ่มต้นแอปเล่นตรงจาก <audio> เหมือนเดิม)

export const EQ_FREQS = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
export const EQ_PRESETS: Record<string, number[]> = {
  flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  bass: [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
  treble: [0, 0, 0, 0, 0, 1, 2, 4, 5, 6],
  vocal: [-2, -2, -1, 1, 3, 4, 3, 1, 0, -1],
  loudness: [5, 4, 1, 0, -1, -1, 0, 1, 4, 5],
  rock: [4, 3, 1, -1, -2, -1, 1, 3, 4, 4],
  pop: [-1, 1, 3, 4, 3, 0, -1, -1, 1, 2]
};

export class Fx {
  ctx = new AudioContext();
  master = this.ctx.createGain();
  deckGain: GainNode[] = [];
  private filters: BiquadFilterNode[] = [];
  private pan = this.ctx.createStereoPanner();
  private monoNode = this.ctx.createGain();

  constructor(els: HTMLAudioElement[]) {
    const c = this.ctx;
    this.filters = EQ_FREQS.map((f) => {
      const b = c.createBiquadFilter();
      b.type = 'peaking';
      b.frequency.value = f;
      b.Q.value = 1.1;
      return b;
    });
    // ลำดับ: deck → deckGain → EQ → pan → master → mono → ออก
    const head = c.createGain();
    this.filters.reduce<AudioNode>((a, b) => a.connect(b), head).connect(this.pan);
    this.pan.connect(this.master).connect(this.monoNode).connect(c.destination);
    this.monoNode.channelCount = 2;
    for (const el of els) {
      const g = c.createGain();
      c.createMediaElementSource(el).connect(g).connect(head);
      this.deckGain.push(g);
    }
    this.deckGain[1].gain.value = 0;
  }

  resume() { if (this.ctx.state === 'suspended') void this.ctx.resume(); }
  setMaster(v: number) { this.master.gain.setTargetAtTime(v, this.ctx.currentTime, 0.01); }
  setEq(gains: number[] | null) {
    this.filters.forEach((f, i) => f.gain.setTargetAtTime(gains ? gains[i] ?? 0 : 0, this.ctx.currentTime, 0.02));
  }
  setBalance(v: number) { this.pan.pan.value = Math.max(-1, Math.min(1, v)); }
  setMono(on: boolean) {
    // ช่องเดียวแบบ explicit = มิกซ์ซ้ายขวารวมกัน แล้วส่งออกสองข้างเท่ากัน
    this.monoNode.channelCount = on ? 1 : 2;
    this.monoNode.channelCountMode = on ? 'explicit' : 'max';
  }
  /** สลับเสียงจากเด็คหนึ่งไปอีกเด็กภายใน sec วินาที */
  crossfade(from: number, to: number, sec: number) {
    const t = this.ctx.currentTime, a = this.deckGain[from].gain, b = this.deckGain[to].gain;
    a.cancelScheduledValues(t); b.cancelScheduledValues(t);
    a.setValueAtTime(a.value, t); b.setValueAtTime(0, t);
    a.linearRampToValueAtTime(0, t + sec); b.linearRampToValueAtTime(1, t + sec);
  }
  /** ตั้งให้เด็ค active ดังเต็ม อีกเด็คเงียบ (ใช้ตอนกดเปลี่ยนเพลงเอง) */
  solo(active: number) {
    const t = this.ctx.currentTime;
    this.deckGain.forEach((g, i) => { g.gain.cancelScheduledValues(t); g.gain.setValueAtTime(i === active ? 1 : 0, t); });
  }
}

/** ทดสอบว่าอ่านไฟล์แบบ CORS ได้ไหม (ต้องได้ ไม่งั้นเสียงจะเงียบเมื่อผ่านกราฟ) */
export async function corsOk(url: string): Promise<boolean> {
  try {
    const r = await fetch(url, { headers: { Range: 'bytes=0-1' } });
    return r.ok || r.status === 206;
  } catch {
    return false;
  }
}

/** หาความเงียบต้น/ท้ายเพลง (วินาที) จากเสียงที่ถอดรหัสแล้ว; เกณฑ์ -50 dBFS */
export function silenceEdges(chs: ArrayLike<number>[], sr: number, thresh = 10 ** (-50 / 20)): { start: number; end: number } {
  const n = chs[0]?.length ?? 0;
  const loud = (i: number) => chs.some((c) => Math.abs(c[i]) > thresh);
  let a = 0, b = n - 1;
  while (a < n && !loud(a)) a++;
  if (a >= n) return { start: 0, end: 0 };
  while (b > a && !loud(b)) b--;
  return { start: a / sr, end: Math.max(0, (n - 1 - b) / sr) };
}
