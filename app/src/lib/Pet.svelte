<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, emit } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow, cursorPosition, currentMonitor, LogicalSize, PhysicalPosition } from '@tauri-apps/api/window';
  import { Menu } from '@tauri-apps/api/menu';
  import { fade } from 'svelte/transition';
  import { i18n, tr } from '$lib/i18n.svelte';

  // หน้าต่างตัวละคร: โปร่งใส ขนาดพอดีรูป อยู่บนสุด
  // ส่วนโปร่งใสของรูปคลิกทะลุได้: ปกติหน้าต่างไม่รับเมาส์ แล้วเช็กตำแหน่งเมาส์เองทุก 50ms ว่าอยู่บนพิกเซลทึบไหม
  type Cfg = { idle: string; hover: string; press: string; size: number; locked: boolean; bounce: boolean; playing: boolean; bpm: number; x?: number; y?: number; side?: 'left' | 'right'; lang?: 'en' | 'th'; motion: boolean; life?: boolean; ly?: boolean; line?: string };
  const win = getCurrentWindow();
  let cfg = $state<Cfg | null>(null);
  let urls = $state<{ idle: string; hover: string; press: string } | null>(null);
  type Box = { x: number; y: number; w: number; h: number; nw: number; nh: number };
  let ratio = $state(1); // สูง/กว้าง ของตัวละครจริง (หลังตัดขอบโปร่งใส)
  let box = $state<Box | null>(null);
  let hover = $state(false), pressed = $state(false);
  let pressAt = 0, releaseT = 0, dragging = false, downAt: { x: number; y: number } | null = null;
  const img: Record<'idle' | 'hover' | 'press', HTMLImageElement | undefined> = { idle: undefined, hover: undefined, press: undefined };
  const pose = $derived<'idle' | 'hover' | 'press'>(pressed ? 'press' : hover ? 'hover' : 'idle');

  // โหลดรูปเป็น blob (ต้นทางเดียวกัน) เพื่อให้อ่านพิกเซลบน canvas ได้
  let loadedKey = '';
  async function loadImages(c: Cfg) {
    const key = `${c.idle}|${c.hover}|${c.press}`;
    if (key === loadedKey) return;
    loadedKey = key;
    const blob = async (p: string) => URL.createObjectURL(new Blob([await invoke<ArrayBuffer>('pet_bytes', { path: p })]));
    const [a, b, d] = await Promise.all([blob(c.idle), blob(c.hover), blob(c.press)]);
    if (urls) for (const u of [urls.idle, urls.hover, urls.press]) URL.revokeObjectURL(u);
    urls = { idle: a, hover: b, press: d };
    box = null;
    const found = /\.(gif|webp)$/i.test(c.idle + c.hover + c.press) ? null : await trimBox([a, b, d]); // ภาพขยับได้ไม่ตัด: เฟรมแรกไม่ใช่ตัวแทนทั้งภาพ
    if (found) { box = found; ratio = found.h / found.w; }
    else {
      const probe = new Image();
      await new Promise<void>((res) => { probe.onload = () => res(); probe.onerror = () => res(); probe.src = a; });
      ratio = probe.naturalHeight / Math.max(1, probe.naturalWidth) || 1;
    }
    fit();
  }
  // หาขอบเขตส่วนที่ไม่โปร่งใสของรูปทั้งสาม (รวมกัน เพื่อให้สลับท่าแล้วตัวไม่เลื่อน) แล้วตัดขอบโปร่งใสรอบๆ ทิ้ง
  // ผลคือเท้าชิดขอบล่างของหน้าต่าง หัวชิดบับเบิล และมุมจอชิดตัวละครจริง ไม่ใช่ขอบรูป
  async function trimBox(srcs: string[]): Promise<Box | null> {
    const g = document.createElement('canvas').getContext('2d', { willReadFrequently: true });
    if (!g) return null;
    let x0 = Infinity, y0 = Infinity, x1 = -1, y1 = -1, nw = 0, nh = 0;
    for (const u of srcs) {
      const im = new Image();
      try { await new Promise<void>((res, rej) => { im.onload = () => res(); im.onerror = () => rej(); im.src = u; }); } catch { return null; }
      if (!im.naturalWidth || !im.naturalHeight) return null;
      if (nw && (im.naturalWidth !== nw || im.naturalHeight !== nh)) return null; // ขนาดรูปไม่เท่ากัน: ไม่ตัด เพื่อไม่ให้ตำแหน่งเพี้ยน
      nw = im.naturalWidth; nh = im.naturalHeight;
      const sc = Math.min(1, 1024 / Math.max(nw, nh)), cw = Math.max(1, Math.round(nw * sc)), ch = Math.max(1, Math.round(nh * sc));
      g.canvas.width = cw; g.canvas.height = ch;
      g.clearRect(0, 0, cw, ch);
      g.drawImage(im, 0, 0, cw, ch);
      let d: Uint8ClampedArray;
      try { d = g.getImageData(0, 0, cw, ch).data; } catch { return null; }
      for (let y = 0; y < ch; y++) for (let x = 0; x < cw; x++) {
        if (d[(y * cw + x) * 4 + 3] > 24) { if (x < x0) x0 = x / sc; if (x > x1) x1 = (x + 1) / sc; if (y < y0) y0 = y / sc; if (y > y1) y1 = (y + 1) / sc; }
      }
    }
    if (x1 < 0) return null;
    const bx = Math.max(0, Math.floor(x0)), by = Math.max(0, Math.floor(y0)), bw = Math.min(nw, Math.ceil(x1)) - bx, bh = Math.min(nh, Math.ceil(y1)) - by;
    if (bw < 8 || bh < 8) return null;
    if (bw >= nw && bh >= nh) return null; // ไม่มีขอบโปร่งใสให้ตัด
    return { x: bx, y: by, w: bw, h: bh, nw, nh };
  }
  const w = () => Math.round(cfg?.size ?? 200);
  const h = () => Math.round(w() * ratio);
  // ถ้าเปิดบับเบิลเนื้อเพลง หน้าต่างจะสูงขึ้น/กว้างขึ้นเพื่อเว้นที่ด้านบน โดยตัวละครอยู่ล่างกึ่งกลางเหมือนเดิม
  const PAD_TOP = 64;
  const padTop = () => (cfg?.ly ? PAD_TOP : 0);
  const winW = () => (cfg?.ly ? Math.max(w(), 280) : w());
  const winH = () => h() + padTop();
  const padX = () => (winW() - w()) / 2;
  async function fit(anchorBottom = false) {
    if (!cfg) return;
    if (anchorBottom) {
      // ย่อ/ขยายโดยให้เท้าอยู่ที่เดิม (กึ่งกลางล่าง)
      const sf = await win.scaleFactor(), pos = await win.outerPosition(), old = await win.outerSize();
      const nw = winW() * sf, nh = winH() * sf;
      await win.setPosition(new PhysicalPosition(Math.round(pos.x + (old.width - nw) / 2), Math.round(pos.y + old.height - nh)));
    }
    await win.setSize(new LogicalSize(winW(), winH()));
  }
  async function snap(side: 'left' | 'right') {
    const m = await currentMonitor();
    if (!m) return;
    const sf = m.scaleFactor, wa = m.workArea ?? { position: m.position, size: m.size };
    const ww = winW() * sf, hh = winH() * sf, pad = 16 * sf;
    const x = side === 'left' ? wa.position.x + pad : wa.position.x + wa.size.width - ww - pad;
    await win.setPosition(new PhysicalPosition(Math.round(x), Math.round(wa.position.y + wa.size.height - hh)));
  }

  // ---------- คลิกทะลุส่วนโปร่งใส ----------
  const cv = document.createElement('canvas');
  const cx = cv.getContext('2d', { willReadFrequently: true });
  let ignoring: boolean | null = null;
  function opaqueAt(wx: number, wy: number): boolean {
    const x = wx - padX(), y = wy - padTop();
    const el = img[pose];
    if (!cx || !el || !el.complete || x < 0 || y < 0 || x >= w() || y >= h()) return false;
    try {
      cv.width = 1; cv.height = 1;
      cx.clearRect(0, 0, 1, 1);
      const kx = (box ? box.w : el.naturalWidth) / w(), ky = (box ? box.h : el.naturalHeight) / h();
      cx.drawImage(el, (box?.x ?? 0) + x * kx, (box?.y ?? 0) + y * ky, Math.max(1, kx), Math.max(1, ky), 0, 0, 1, 1);
      return cx.getImageData(0, 0, 1, 1).data[3] > 24;
    } catch {
      return true; // อ่านพิกเซลไม่ได้: ถือว่าทั้งกรอบเป็นตัวละคร
    }
  }
  async function setIgnore(v: boolean) {
    if (ignoring === v) return;
    ignoring = v;
    await win.setIgnoreCursorEvents(v).catch(() => {});
  }
  async function poll() {
    if (!cfg || !urls) return;
    try {
      const [c, p, sf] = await Promise.all([cursorPosition(), win.outerPosition(), win.scaleFactor()]);
      const on = opaqueAt((c.x - p.x) / sf, (c.y - p.y) / sf);
      if (!pressed) hover = on;
      if (on && sleeping) rest();
      setIgnore(!on && !pressed);
    } catch { /* หน้าต่างกำลังปิด */ }
  }

  // ---------- เมาส์ ----------
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    clearTimeout(releaseT);
    if (sleeping) rest();
    pressed = true; pressAt = performance.now(); downAt = { x: e.screenX, y: e.screenY }; dragging = false;
  }
  function move(e: PointerEvent) {
    if (!downAt || dragging || cfg?.locked) return;
    if (Math.hypot(e.screenX - downAt.x, e.screenY - downAt.y) > 4) {
      dragging = true;
      win.startDragging().catch(() => {});
    }
  }
  function up() {
    downAt = null;
    const left = Math.max(0, 300 - (performance.now() - pressAt)); // ภาพตอนคลิกค้างอย่างน้อย 0.3 วิ
    releaseT = setTimeout(() => (pressed = false), left) as unknown as number;
  }
  function wheel(e: WheelEvent) {
    if (!cfg || cfg.locked) return;
    e.preventDefault();
    cfg.size = Math.round(Math.min(800, Math.max(48, cfg.size * (e.deltaY < 0 ? 1.08 : 1 / 1.08))));
    fit(true);
    emit('pet-set', { size: cfg.size });
  }
  async function menu(e: MouseEvent) {
    e.preventDefault();
    if (!cfg) return;
    const m = await Menu.new({
      items: [
        { id: 'lock', text: cfg.locked ? tr('ปลดล็อก') : tr('ล็อกตำแหน่งและขนาด'), action: () => emit('pet-set', { locked: !cfg!.locked }) },
        { id: 'left', text: tr('ไปมุมซ้ายล่าง'), action: () => emit('pet-set', { side: 'left' }) },
        { id: 'right', text: tr('ไปมุมขวาล่าง'), action: () => emit('pet-set', { side: 'right' }) },
        { item: 'Separator' },
        { id: 'hide', text: tr('ซ่อนตัวละคร'), action: () => emit('pet-set', { hide: true }) }
      ]
    });
    await m.popup();
  }

  // ---------- ท่าทางมีชีวิต ----------
  // หยุดเพลงนาน -> หลับ (มืดลง + Zzz) ชี้/คลิก/เล่นเพลงแล้วตื่น
  const SLEEP_MS = 60_000;
  let sleeping = $state(false), sleepT: ReturnType<typeof setTimeout> | undefined;
  function rest() {
    clearTimeout(sleepT);
    sleeping = false;
    if (cfg?.life && !cfg.playing) sleepT = setTimeout(() => (sleeping = true), SLEEP_MS);
  }
  $effect(() => {
    void [cfg?.life, cfg?.playing];
    rest();
    return () => clearTimeout(sleepT);
  });
  const breathe = $derived(!!cfg && cfg.life && !cfg.playing && cfg.motion);

  // หัวใจลอยตอนกดถูกใจ
  let hearts = $state<{ id: number; x: number; d: number }[]>([]);
  let heartId = 0;
  function popHearts() {
    if (!cfg?.life) return;
    const add = [0, 1, 2].map((i) => ({ id: ++heartId, x: 30 + Math.random() * 40, d: i * 140 }));
    hearts = [...hearts, ...add];
    const ids = add.map((x) => x.id);
    setTimeout(() => (hearts = hearts.filter((x) => !ids.includes(x.id))), 2000);
  }

  // เด้งตามความดังจริง: ยุบ-ยืดจากเท้า (ไม่เกินขอบหน้าต่าง) ถ้าไม่มีค่าเสียงเข้ามา ใช้เด้งตาม BPM แทน
  let sprite = $state<HTMLDivElement>();
  let target = 0, level = 0, lastLv = 0, reactive = $state(false);
  onMount(() => {
    let raf = 0;
    const loop = () => {
      const fresh = !!cfg && cfg.bounce && cfg.playing && cfg.motion && performance.now() - lastLv < 600;
      if (fresh !== reactive) reactive = fresh;
      if (sprite) {
        if (fresh) {
          level += (target - level) * (target > level ? 0.6 : 0.12);
          sprite.style.transform = `scale(${(1.025 - 0.025 * level).toFixed(4)}, ${(0.93 + 0.07 * level).toFixed(4)})`;
        } else if (sprite.style.transform) {
          level = 0;
          sprite.style.transform = '';
        }
      }
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  });

  onMount(() => {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
    let moveT = 0, first = true;
    const subs = [
      listen<Cfg>('pet-cfg', async (e) => {
        const prev = cfg;
        cfg = e.payload;
        if (cfg.lang) i18n.lang = cfg.lang;
        await loadImages(cfg);
        if (prev && (prev.size !== cfg.size || prev.ly !== cfg.ly)) fit(true);
        if (first) {
          first = false;
          await fit();
          if (cfg.x != null && cfg.y != null) await win.setPosition(new PhysicalPosition(cfg.x, cfg.y));
          else await snap('right');
          await win.show();
        }
      }),
      listen<'left' | 'right'>('pet-snap', (e) => snap(e.payload)),
      listen<number>('pet-level', (e) => { target = e.payload; lastLv = performance.now(); }),
      listen('pet-heart', () => popHearts()),
      // ลากเสร็จ (Windows ไม่ส่ง pointerup หลังลากหน้าต่าง): หยุดขยับ 250ms แล้วถือว่าปล่อยเมาส์ + บันทึกตำแหน่ง
      win.onMoved(({ payload }) => {
        clearTimeout(moveT);
        moveT = setTimeout(() => {
          if (dragging) { dragging = false; up(); }
          emit('pet-set', { x: payload.x, y: payload.y });
        }, 250) as unknown as number;
      })
    ];
    const t = setInterval(poll, 50);
    setIgnore(true);
    emit('pet-ready');
    return () => { clearInterval(t); subs.forEach((s) => s.then((f) => f())); };
  });
  const beat = $derived(cfg && cfg.bounce && cfg.playing && cfg.motion && !reactive ? 60 / Math.min(200, Math.max(60, cfg.bpm || 100)) : 0);
</script>

<svelte:window onpointerup={up} />
{#if urls && cfg}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="pet" class:locked={cfg.locked} onpointerdown={down} onpointermove={move} onwheel={wheel} oncontextmenu={menu}>
    {#if cfg.ly}
      <div class="bubblebox">
        {#key cfg.line}
          {#if cfg.line}<div class="bubble" in:fade={{ duration: cfg.motion ? 160 : 0 }}><span>{cfg.line}</span></div>{/if}
        {/key}
      </div>
    {/if}
    <div class="stage" style:left="{padX()}px" style:top="{padTop()}px" style:width="{w()}px" style:height="{h()}px">
      <div class="sprite" bind:this={sprite} style:--beat="{beat}s" class:bounce={beat > 0} class:breathe class:slow={sleeping} class:asleep={sleeping && !hover && !pressed}>
        {#each ['idle', 'hover', 'press'] as const as k (k)}
          <img bind:this={img[k]} src={urls[k]} alt="" draggable="false" class:on={pose === k} class:crop={!!box}
            style={box ? `width:${(box.nw / box.w) * 100}%;height:${(box.nh / box.h) * 100}%;left:${(-box.x / box.w) * 100}%;top:${(-box.y / box.h) * 100}%` : ''} />
        {/each}
      </div>
      {#if sleeping && !hover && !pressed}
        <div class="zzz" class:still={!cfg.motion} aria-hidden="true"><i>Z</i><i>z</i><i>z</i></div>
      {/if}
      {#each hearts as ht (ht.id)}
        <b class="heart" class:still={!cfg.motion} style:left="{ht.x}%" style:animation-delay="{ht.d}ms" style:--rise="{Math.round(h() * 0.3)}px" aria-hidden="true">&#9829;</b>
      {/each}
    </div>
  </div>
{/if}

<style>
  :global(html, body) { margin: 0; background: transparent !important; overflow: hidden; user-select: none; }
  .pet { position: fixed; inset: 0; cursor: grab; }
  .pet.locked { cursor: pointer; }
  .pet:active { cursor: grabbing; }
  .pet.locked:active { cursor: pointer; }
  .stage { position: absolute; }
  .sprite { position: absolute; inset: 0; overflow: hidden; transform-origin: 50% 100%; transition: filter 0.8s ease; }
  .sprite.asleep { filter: brightness(0.62) saturate(0.75); }
  /* วางรูปทั้งสามซ้อนกันแล้วสลับด้วย visibility: เปลี่ยนท่าทันที ไม่มีจังหวะโหลดรูป */
  img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; visibility: hidden; pointer-events: none; -webkit-user-drag: none; }
  img.on { visibility: visible; }
  img.crop { inset: auto; object-fit: fill; }
  /* เด้งตามเพลง (สำรอง เมื่ออ่านระดับเสียงไม่ได้): ยุบ-ยืดจากเท้า หนึ่งครั้งต่อจังหวะ */
  .bounce { animation: bob var(--beat) ease-in-out infinite; }
  @keyframes bob { 0%, 100% { transform: none; } 20% { transform: scale(1.02, 0.94); } 50% { transform: scale(0.98, 1); } }
  /* หายใจ: พองเบาๆ ช้าๆ จากเท้า (ไม่เกินขอบหน้าต่าง) หลับแล้วช้าลง */
  .breathe { animation: breathe 4.2s ease-in-out infinite; }
  .breathe.slow { animation-duration: 6.5s; }
  @keyframes breathe { 0%, 100% { transform: scale(0.992, 0.982); } 50% { transform: scale(1, 1); } }
  .zzz { position: absolute; right: 8%; top: 6%; width: 3em; height: 3em; font: 800 18px/1 system-ui, sans-serif; color: #fff; pointer-events: none; }
  .zzz i { position: absolute; left: 0; bottom: 0; font-style: normal; opacity: 0; text-shadow: 0 0 3px #000, 0 1px 3px #000; animation: zz 3s ease-in-out infinite; }
  .zzz i:nth-child(2) { animation-delay: 1s; font-size: 0.85em; }
  .zzz i:nth-child(3) { animation-delay: 2s; font-size: 0.7em; }
  .zzz.still i { animation: none; opacity: 0.9; }
  .zzz.still i:nth-child(2) { left: 0.9em; bottom: 0.9em; }
  .zzz.still i:nth-child(3) { left: 1.7em; bottom: 1.7em; }
  @keyframes zz { 0% { opacity: 0; transform: translate(0, 0); } 20% { opacity: 1; } 100% { opacity: 0; transform: translate(1.6em, -2em); } }
  .heart { position: absolute; top: 28%; font-size: 22px; line-height: 1; color: #ff5d7a; text-shadow: 0 0 4px rgba(0, 0, 0, 0.55); opacity: 0; pointer-events: none; animation: rise 1.5s ease-out both; }
  .heart.still { animation: fadeh 1.5s ease-out both; }
  @keyframes rise { 0% { opacity: 0; transform: translateY(8px) scale(0.6); } 20% { opacity: 1; transform: translateY(0) scale(1.1); } 100% { opacity: 0; transform: translateY(calc(var(--rise) * -1)) scale(1); } }
  @keyframes fadeh { 0%, 100% { opacity: 0; } 25%, 60% { opacity: 1; } }
  .bubblebox { position: absolute; left: 6px; right: 6px; top: 0; height: 54px; display: flex; align-items: flex-end; justify-content: center; pointer-events: none; }
  .bubble { position: relative; max-width: 100%; padding: 6px 12px; border-radius: 12px; background: rgba(18, 18, 22, 0.86); color: #fff; font: 600 13px/1.3 system-ui, sans-serif; text-align: center; box-shadow: 0 2px 8px rgba(0, 0, 0, 0.35); border: 1px solid rgba(255, 255, 255, 0.2); }
  .bubble span { display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; overflow-wrap: anywhere; }
  .bubble::after { content: ''; position: absolute; left: 50%; bottom: -5px; margin-left: -5px; border: 5px solid transparent; border-bottom: 0; border-top-color: rgba(18, 18, 22, 0.86); }
</style>
