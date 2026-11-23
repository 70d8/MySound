<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';
  import { tr } from '$lib/i18n.svelte';

  // การ์ดแชร์เนื้อเพลง: เลือกบรรทัด / สไตล์ / สัดส่วน / สีพื้น แล้วบันทึกเป็น PNG หรือคัดลอก
  let { title, artist, album, cover, lines, first, accent, onclose, onmsg }: {
    title: string; artist: string; album: string; cover: string | null; lines: string[]; first: number; accent: string;
    onclose: () => void; onmsg: (m: string) => void;
  } = $props();

  type Style = 'genius' | 'cover' | 'poster' | 'minimal';
  type Size = 'square' | 'portrait' | 'story';
  const STYLES: [Style, string][] = [['genius', 'สีพื้น (แบบ Genius)'], ['cover', 'ภาพปกเบลอ'], ['poster', 'โปสเตอร์'], ['minimal', 'เรียบ']];
  const SIZES: [Size, string, number, number][] = [['square', '1:1', 1080, 1080], ['portrait', '4:5', 1080, 1350], ['story', '9:16', 1080, 1920]];
  const MAXPICK = 6;
  const FONT = "Sarabun, 'Noto Sans Thai', system-ui, sans-serif";

  let dlg = $state<HTMLDialogElement>();
  let cv = $state<HTMLCanvasElement>();
  let style = $state<Style>('genius');
  let size = $state<Size>('square');
  let bgKey = $state<'auto' | 'accent' | 'black' | 'white' | 'custom'>('auto');
  let custom = $state('#7c3aed');
  let showCover = $state(true);
  let showBrand = $state(true);
  let manual = $state('');
  // ค่าเริ่มต้นอ่านครั้งเดียว: บรรทัดที่กำลังร้อง + อีก 2 บรรทัดถัดไป
  let picked = $state<number[]>(untrack(() => {
    const st = Math.max(0, Math.min(first, Math.max(0, lines.length - 1)));
    return lines.length ? Array.from({ length: Math.min(3, lines.length - st) }, (_, i) => st + i) : [];
  }));
  let img = $state<HTMLImageElement | null>(null);
  let dom = $state('#334155'); // สีเด่นจากปก
  let fontTick = $state(0);
  let busy = $state(false);

  onMount(() => {
    dlg?.showModal();
    if (cover) {
      const i = new Image();
      i.crossOrigin = 'anonymous';
      i.onload = () => { img = i; dom = dominant(i) ?? accent; };
      i.src = cover;
    } else dom = accent;
    void document.fonts.load(`700 40px Sarabun`).then(() => fontTick++).catch(() => {});
  });

  /** สีเด่นของปก: เฉลี่ยพิกเซลโดยให้น้ำหนักตามความสด ตัดสีที่ขาว/ดำสนิทออก */
  function dominant(i: HTMLImageElement): string | null {
    try {
      const c = document.createElement('canvas'); c.width = c.height = 24;
      const g = c.getContext('2d', { willReadFrequently: true })!;
      g.drawImage(i, 0, 0, 24, 24);
      const d = g.getImageData(0, 0, 24, 24).data;
      let r = 0, gg = 0, b = 0, w = 0;
      for (let p = 0; p < d.length; p += 4) {
        const mx = Math.max(d[p], d[p + 1], d[p + 2]), mn = Math.min(d[p], d[p + 1], d[p + 2]);
        if (mx < 30 || mn > 235) continue;
        const k = 0.2 + (mx - mn) / 255;
        r += d[p] * k; gg += d[p + 1] * k; b += d[p + 2] * k; w += k;
      }
      return w ? `rgb(${Math.round(r / w)},${Math.round(gg / w)},${Math.round(b / w)})` : null;
    } catch { return null; }
  }
  const lum = (css: string) => {
    const c = document.createElement('canvas'); c.width = c.height = 1;
    const g = c.getContext('2d')!; g.fillStyle = css; g.fillRect(0, 0, 1, 1);
    const [r, gr, b] = g.getImageData(0, 0, 1, 1).data;
    return (0.2126 * r + 0.7152 * gr + 0.0722 * b) / 255;
  };
  const bgColor = $derived(bgKey === 'auto' ? dom : bgKey === 'accent' ? accent : bgKey === 'black' ? '#0c0c0e' : bgKey === 'white' ? '#f6f4ef' : custom);

  const seg = typeof Intl !== 'undefined' && 'Segmenter' in Intl ? new Intl.Segmenter(undefined, { granularity: 'word' }) : null;
  /** ตัดบรรทัดตามคำ (รองรับภาษาไทยที่ไม่มีเว้นวรรค) */
  function wrap(g: CanvasRenderingContext2D, text: string, maxW: number): string[] {
    const words = seg ? [...seg.segment(text)].map((s) => s.segment) : text.split(/(\s+)/);
    const out: string[] = [];
    let cur = '';
    for (const w of words) {
      const t = cur + w;
      if (cur && g.measureText(t.trimEnd()).width > maxW) { out.push(cur.trimEnd()); cur = w.trimStart(); } else cur = t;
    }
    if (cur.trim()) out.push(cur.trimEnd());
    return out;
  }
  const quoteLines = $derived(lines.length ? [...picked].sort((a, b) => a - b).map((i) => lines[i]) : manual.split('\n').map((x) => x.trim()).filter(Boolean));

  type Box = { x: number; y: number; w: number; h: number };
  /** วาดเนื้อเพลงเป็นย่อหน้า (1 บรรทัดเพลง = 1 ย่อหน้า) ปรับขนาดอัตโนมัติให้พอดีกล่อง */
  function drawQuote(g: CanvasRenderingContext2D, qs: string[], b: Box, color: string, align: CanvasTextAlign, maxSize: number, vAlign: 'top' | 'middle') {
    let fs = maxSize, rows: string[][] = [], lh = 0, gap = 0, total = 0;
    for (; fs >= 28; fs -= 4) {
      g.font = `700 ${fs}px ${FONT}`;
      rows = qs.map((q) => wrap(g, q, b.w));
      lh = fs * 1.28; gap = fs * 0.34;
      total = rows.reduce((s, r) => s + r.length * lh, 0) + gap * Math.max(0, rows.length - 1);
      if (total <= b.h) break;
    }
    g.fillStyle = color; g.textAlign = align; g.textBaseline = 'alphabetic';
    let y = b.y + (vAlign === 'middle' ? Math.max(0, (b.h - total) / 2) : 0) + fs * 0.98;
    const x = align === 'center' ? b.x + b.w / 2 : b.x;
    for (const r of rows) { for (const l of r) { g.fillText(l, x, y); y += lh; } y += gap; }
  }
  function fitText(g: CanvasRenderingContext2D, txt: string, size: number, weight: number, maxW: number) {
    let s = size;
    g.font = `${weight} ${s}px ${FONT}`;
    while (g.measureText(txt).width > maxW && s > 20) { s -= 2; g.font = `${weight} ${s}px ${FONT}`; }
    return txt;
  }
  function rr(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) { g.beginPath(); g.roundRect(x, y, w, h, r); }
  function drawCover(g: CanvasRenderingContext2D, x: number, y: number, s: number, r: number) {
    g.save(); rr(g, x, y, s, s, r); g.clip();
    if (img) g.drawImage(img, x, y, s, s); else { g.fillStyle = accent; g.fillRect(x, y, s, s); }
    g.restore();
  }

  function draw() {
    void fontTick;
    if (!cv) return;
    const [, , W, H] = SIZES.find((s) => s[0] === size)!;
    cv.width = W; cv.height = H;
    const g = cv.getContext('2d')!;
    const M = 84; // ขอบ
    const cov = showCover && !!img; // ไม่มีรูปปก = ไม่เว้นที่ว่างให้
    const qs = quoteLines;
    const bg = style === 'minimal' && bgKey === 'auto' ? '#0c0c0e' : bgColor;
    const dark = style === 'cover' || lum(bg) < 0.58;
    const fg = style === 'cover' ? '#fff' : dark ? '#fff' : '#111';
    const sub = style === 'cover' || dark ? 'rgba(255,255,255,.72)' : 'rgba(0,0,0,.62)';
    // พื้นหลัง
    g.fillStyle = bg; g.fillRect(0, 0, W, H);
    if (style === 'cover') {
      if (img) { g.filter = 'blur(70px) brightness(0.55) saturate(1.2)'; g.drawImage(img, -260, -260, W + 520, H + 520); g.filter = 'none'; }
      g.fillStyle = 'rgba(0,0,0,.28)'; g.fillRect(0, 0, W, H);
    }
    const brand = () => {
      if (!showBrand) return;
      g.textAlign = 'right'; g.textBaseline = 'alphabetic'; g.font = `700 28px ${FONT}`; g.fillStyle = sub; g.fillText('MySound', W - M, H - M + 6);
    };
    const meta = (x: number, y: number, align: CanvasTextAlign, maxW: number) => {
      g.textAlign = align; g.textBaseline = 'alphabetic';
      fitText(g, title, 40, 700, maxW); g.fillStyle = fg; g.fillText(title, x, y);
      fitText(g, artist, 30, 400, maxW); g.fillStyle = sub; g.fillText(artist, x, y + 42);
    };

    if (style === 'genius') {
      // ข้อความชิดซ้ายตัวใหญ่บนพื้นสี ด้านล่างเป็นปก + ชื่อเพลง/ศิลปิน (คล้ายการ์ดเนื้อเพลงของ Genius)
      const foot = 150;
      g.font = `700 150px ${FONT}`; g.fillStyle = sub; g.textAlign = 'left'; g.fillText('“', M - 6, M + 110);
      drawQuote(g, qs, { x: M, y: M + 110, w: W - M * 2, h: H - M * 2 - foot - 110 }, fg, 'left', 96, 'top');
      const y0 = H - M - foot + 20;
      let tx = M;
      if (cov) { drawCover(g, M, y0, 112, 14); tx = M + 112 + 28; }
      meta(tx, y0 + 48, 'left', W - tx - M - (showBrand ? 170 : 0));
      brand();
    } else if (style === 'cover') {
      drawQuote(g, qs, { x: M, y: M + 40, w: W - M * 2, h: H - M * 2 - 300 }, fg, 'center', 92, 'middle');
      if (cov) drawCover(g, W / 2 - 60, H - M - 250, 120, 18);
      meta(W / 2, H - M - (cov ? 90 : 120), 'center', W - M * 2);
      brand();
    } else if (style === 'poster') {
      const S = Math.min(W - M * 2, size === 'story' ? 760 : size === 'portrait' ? 620 : 470);
      if (cov) drawCover(g, (W - S) / 2, M, S, 28);
      const top = cov ? M + S + 56 : M + 20;
      drawQuote(g, qs, { x: M, y: top, w: W - M * 2, h: H - top - M - 130 }, fg, 'center', 70, 'middle');
      meta(W / 2, H - M - 56, 'center', W - M * 2);
      if (showBrand) { g.textAlign = 'center'; g.font = `700 24px ${FONT}`; g.fillStyle = sub; g.fillText('MySound', W / 2, H - M + 14); }
    } else {
      // เรียบ: ตัวอักษรล้วน เส้นสีเน้น
      g.fillStyle = accent; g.fillRect(M, M, 96, 8);
      drawQuote(g, qs, { x: M, y: M + 60, w: W - M * 2, h: H - M * 2 - 220 }, fg, 'left', 88, 'middle');
      meta(M, H - M - 60, 'left', W - M * 2 - (showBrand ? 170 : 0));
      brand();
    }
  }
  $effect(() => {
    // อ่านค่าทั้งหมดที่ใช้วาดเพื่อให้วาดใหม่เมื่อตัวใดเปลี่ยน
    void [style, size, bgKey, custom, showCover, showBrand, picked.length, quoteLines.join('\n'), img, dom, bgColor, cv];
    draw();
  });

  function toggle(i: number) {
    if (picked.includes(i)) picked = picked.filter((x) => x !== i);
    else if (picked.length < MAXPICK) picked = [...picked, i];
    else onmsg(tr('เลือกได้สูงสุด {0} บรรทัด', MAXPICK));
  }
  const fname = () => `${title.replace(/[<>:"/\\|?*]+/g, '_').slice(0, 80) || 'MySound'}.png`;
  async function doSave() {
    if (!cv || busy) return;
    const f = await save({ defaultPath: fname(), filters: [{ name: 'PNG', extensions: ['png'] }] });
    if (!f) return;
    busy = true;
    try { await invoke('save_png', { path: f, b64: cv.toDataURL('image/png') }); onmsg(tr('บันทึกการ์ดแล้ว')); dlg?.close(); } catch (e) { onmsg(tr(String(e))); }
    busy = false;
  }
  async function doCopy() {
    if (!cv) return;
    try {
      const blob = await new Promise<Blob | null>((r) => cv!.toBlob(r, 'image/png'));
      if (!blob) throw new Error('no blob');
      await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
      onmsg(tr('คัดลอกการ์ดแล้ว'));
    } catch { onmsg(tr('คัดลอกรูปไม่ได้ ลองบันทึกเป็นไฟล์แทน')); }
  }
</script>

<dialog bind:this={dlg} class="sc" aria-labelledby="sc-h" onclose={onclose}>
  <h2 id="sc-h">{tr('การ์ดแชร์')}</h2>
  <div class="body">
    <div class="pv"><canvas bind:this={cv} aria-label={tr('ตัวอย่างการ์ด')}></canvas></div>
    <div class="ctl">
      <div class="grp" role="group" aria-label={tr('สไตล์')}>
        {#each STYLES as [k, label] (k)}
          <button type="button" class="chip" class:on={style === k} aria-pressed={style === k} onclick={() => (style = k)}>{tr(label)}</button>
        {/each}
      </div>
      <div class="grp" role="group" aria-label={tr('สัดส่วน')}>
        {#each SIZES as [k, label] (k)}
          <button type="button" class="chip" class:on={size === k} aria-pressed={size === k} onclick={() => (size = k)}>{label}</button>
        {/each}
      </div>
      {#if style !== 'cover'}
        <div class="grp" role="group" aria-label={tr('สีพื้น')}>
          <button type="button" class="sw" class:on={bgKey === 'auto'} style:background={dom} aria-label={tr('สีจากปก')} onclick={() => (bgKey = 'auto')}></button>
          <button type="button" class="sw" class:on={bgKey === 'accent'} style:background={accent} aria-label={tr('สีธีม')} onclick={() => (bgKey = 'accent')}></button>
          <button type="button" class="sw" class:on={bgKey === 'black'} style:background="#0c0c0e" aria-label={tr('ดำ')} onclick={() => (bgKey = 'black')}></button>
          <button type="button" class="sw" class:on={bgKey === 'white'} style:background="#f6f4ef" aria-label={tr('ขาว')} onclick={() => (bgKey = 'white')}></button>
          <input type="color" name="sc-color" aria-label={tr('เลือกสีเอง')} bind:value={custom} oninput={() => (bgKey = 'custom')} />
        </div>
      {/if}
      <div class="grp">
        <label class="chk"><input type="checkbox" name="sc-cover" bind:checked={showCover} /> {tr('แสดงปก')}</label>
        <label class="chk"><input type="checkbox" name="sc-brand" bind:checked={showBrand} /> {tr('ชื่อแอป')}</label>
      </div>
      {#if lines.length}
        <p class="hint">{tr('เลือกบรรทัดที่จะใส่ในการ์ด (สูงสุด {0})', MAXPICK)}</p>
        <div class="lines" role="group" aria-label={tr('บรรทัดเนื้อเพลง')}>
          {#each lines as l, i (i)}
            <button type="button" class="ln" class:on={picked.includes(i)} aria-pressed={picked.includes(i)} onclick={() => toggle(i)}>{l}</button>
          {/each}
        </div>
      {:else}
        <p class="hint">{tr('ยังไม่มีเนื้อเพลง พิมพ์ข้อความที่จะใส่ในการ์ดได้เอง')}</p>
        <textarea name="sc-text" rows="5" placeholder={tr('พิมพ์ข้อความ…')} bind:value={manual}></textarea>
      {/if}
    </div>
  </div>
  <div class="act">
    <button type="button" class="ghost" onclick={() => dlg?.close()}>{tr('ปิด')}</button>
    <span class="grow"></span>
    <button type="button" class="ghost" onclick={doCopy}>{tr('คัดลอกรูป')}</button>
    <button type="button" class="primary" disabled={busy} onclick={doSave}>{tr('บันทึกเป็น PNG…')}</button>
  </div>
</dialog>

<style>
  .sc { margin: auto; width: min(980px, 96vw); max-height: 94vh; padding: 20px 22px; border: 1px solid var(--line); border-radius: 16px; background: var(--bg-raised); color: var(--text); overflow: hidden; }
  .sc::backdrop { background: rgba(0, 0, 0, 0.55); }
  h2 { margin: 0 0 12px; font-size: 18px; }
  .body { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 20px; align-items: start; }
  .pv { display: flex; justify-content: center; align-items: center; background: rgba(0, 0, 0, 0.25); border-radius: 12px; padding: 12px; }
  canvas { max-width: 100%; max-height: min(64vh, 620px); border-radius: 8px; box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35); }
  .ctl { display: flex; flex-direction: column; gap: 12px; max-height: min(64vh, 640px); overflow-y: auto; padding-right: 4px; }
  .grp { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .chip { height: 30px; padding: 0 12px; border-radius: 15px; border: 1px solid var(--line); background: transparent; color: inherit; cursor: pointer; transition: background-color 150ms ease; }
  .chip:hover { background: rgba(255, 255, 255, 0.07); }
  .chip.on { background: var(--accent); border-color: var(--accent); color: var(--on-accent, #000); }
  .sw { width: 28px; height: 28px; border-radius: 50%; border: 2px solid transparent; outline: 1px solid var(--line); cursor: pointer; }
  .sw.on { border-color: var(--text); }
  input[type='color'] { width: 32px; height: 28px; padding: 0; border: 0; background: none; cursor: pointer; }
  .chk { display: inline-flex; gap: 6px; align-items: center; font-size: 13px; cursor: pointer; }
  .hint { margin: 0; font-size: 12.5px; opacity: 0.7; }
  .lines { display: flex; flex-direction: column; gap: 4px; }
  .ln { text-align: left; padding: 7px 10px; border-radius: 8px; border: 1px solid transparent; background: rgba(255, 255, 255, 0.05); color: inherit; cursor: pointer; font-size: 13.5px; line-height: 1.35; }
  .ln:hover { background: rgba(255, 255, 255, 0.1); }
  .ln.on { border-color: var(--accent); background: rgba(255, 255, 255, 0.12); }
  textarea { width: 100%; resize: vertical; padding: 8px 10px; border-radius: 8px; border: 1px solid var(--line); background: rgba(0, 0, 0, 0.2); color: inherit; font: inherit; }
  .ghost, .primary { height: 36px; padding: 0 18px; border-radius: 8px; border: 1px solid var(--line); background: transparent; color: inherit; cursor: pointer; font-weight: 600; transition: transform 140ms ease, background-color 140ms ease; }
  .ghost:hover { background: rgba(255, 255, 255, 0.08); }
  .primary { background: var(--accent); border-color: var(--accent); color: var(--on-accent, #000); font-weight: 700; }
  .primary:disabled { opacity: 0.5; cursor: default; }
  .ghost:active:not(:disabled), .primary:active:not(:disabled) { transform: scale(0.97); }
  .act { display: flex; gap: 8px; align-items: center; margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--line); }
  .grow { flex: 1; }
  :is(.chip, .sw, .ln, .act button):focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (max-width: 760px) { .body { grid-template-columns: 1fr; } .ctl { max-height: none; } }
</style>
