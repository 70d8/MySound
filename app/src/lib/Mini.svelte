<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, emit } from '@tauri-apps/api/event';
  import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
  import Icon from '$lib/Icon.svelte';
  import { fmt, cover } from '$lib/data';
  import { i18n, tr } from '$lib/i18n.svelte';

  // หน้าต่างมินิเพลเยอร์: ไม่มีเสียงของตัวเอง แค่แสดงสถานะที่หน้าต่างหลักส่งมา และส่งคำสั่งกลับไป
  type Np = { has: boolean; title: string; artist: string; playing: boolean; pos: number; dur: number; tint: string; cover: string | null; line?: string; theme?: Record<string, string>; lang?: 'en' | 'th' };
  let s = $state<Np>({ has: false, title: '', artist: '', playing: false, pos: 0, dur: 0, tint: '#1b2a3a', cover: null });
  const cmd = (c: string, v?: number) => emit('np-cmd', { cmd: c, v });
  const win = getCurrentWindow();
  let pinned = $state(true);
  let small = $state(false);
  const togglePin = async () => { pinned = !pinned; await win.setAlwaysOnTop(pinned).catch(() => {}); };
  const toggleSmall = async () => { small = !small; await win.setSize(new LogicalSize(small ? 280 : 360, small ? 72 : 184)).catch(() => {}); };

  onMount(() => {
    const un = listen<Np>('np', (e) => {
      s = e.payload;
      if (s.lang) i18n.lang = s.lang;
      for (const [k, v] of Object.entries(s.theme ?? {})) document.documentElement.style.setProperty(k, v);
    });
    emit('np-ready');
    return () => void un.then((f) => f());
  });
</script>

<svelte:window onkeydown={(e) => e.code === 'Space' && (e.target as HTMLElement).tagName !== 'INPUT' && (e.preventDefault(), cmd('toggle'))} />

<div class="mini" class:small style:--tint={s.tint}>
  <div class="wbtn">
    <button aria-label={pinned ? tr('เลิกปักหมุดไว้บนสุด') : tr('ปักหมุดไว้บนสุด')} aria-pressed={pinned} onclick={togglePin}>{pinned ? '📌' : '📍'}</button>
    <button aria-label={small ? tr('ขยาย') : tr('ย่อ')} onclick={toggleSmall}>{small ? '⤢' : '⤡'}</button>
  </div>
  <span class="cv" style:background={s.cover ? `center / cover url("${s.cover}")` : cover(s.title || 'MySound')}></span>
  <div class="info">
    <b>{s.has ? s.title : tr('ยังไม่ได้เล่นเพลง')}</b>
    <small>{s.artist}</small>
    <input type="range" min="0" max={s.dur} step="1" value={s.pos} oninput={(e) => cmd('seek', +e.currentTarget.value)} aria-label={tr('ตำแหน่งเพลง')} disabled={!s.has} />
    <div class="row">
      <small>{fmt(s.pos)}</small>
      <span class="ctrl">
        <button class="icon" aria-label={tr('ก่อนหน้า')} onclick={() => cmd('prev')}><Icon n="prev" size={18} /></button>
        <button class="playbtn" aria-label={s.playing ? tr('หยุด') : tr('เล่น')} onclick={() => cmd('toggle')}><Icon n={s.playing ? 'pause' : 'play'} size={18} /></button>
        <button class="icon" aria-label={tr('ถัดไป')} onclick={() => cmd('next')}><Icon n="next" size={18} /></button>
      </span>
      <small>{fmt(s.dur)}</small>
    </div>
    {#if s.line}<p class="mly">{s.line}</p>{/if}
  </div>
</div>

<style>
  .mly { margin: 2px 0 0; font-size: 12px; font-weight: 700; text-align: center; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; opacity: 0.9; }
  .small .mly { display: none; }
  .wbtn { position: fixed; top: 4px; right: 6px; display: flex; gap: 2px; z-index: 2; opacity: 0; transition: opacity 120ms; }
  .mini:hover .wbtn { opacity: 1; }
  .wbtn button { all: unset; cursor: pointer; width: 22px; height: 22px; text-align: center; border-radius: 6px; font-size: 12px; }
  .wbtn button:hover { background: rgba(255,255,255,.15); }
  .small .cv { width: 48px; height: 48px; }
  .small input[type='range'], .small .row small { display: none; }
  .mini { display: flex; gap: 12px; height: 100vh; padding: 12px; align-items: center; background: linear-gradient(135deg, color-mix(in srgb, var(--tint) 55%, var(--bg-outer)), var(--bg-base)); }
  .cv { width: 92px; height: 92px; border-radius: 6px; flex: none; }
  .info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  b, small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  small { color: var(--text-dim); font-size: 0.8125rem; font-variant-numeric: tabular-nums; }
  input[type='range'] { width: 100%; height: var(--progress-h); accent-color: var(--accent); margin: 6px 0 2px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .ctrl { display: flex; align-items: center; gap: 6px; }
  .icon { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 50%; color: var(--text-dim); transition: transform 160ms var(--ease-out); }
  .icon:hover { color: var(--text); }
  .playbtn { display: grid; place-items: center; width: 36px; height: 36px; border-radius: 50%; background: var(--text); color: var(--bg-base); transition: transform 160ms var(--ease-out); }
  .icon:active, .playbtn:active { transform: scale(0.95); }
</style>
