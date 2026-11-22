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
