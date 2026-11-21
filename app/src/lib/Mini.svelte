<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, emit } from '@tauri-apps/api/event';
  import Icon from '$lib/Icon.svelte';
  import { fmt, cover } from '$lib/data';
  import { i18n, tr } from '$lib/i18n.svelte';

  // หน้าต่างมินิเพลเยอร์: ไม่มีเสียงของตัวเอง แค่แสดงสถานะที่หน้าต่างหลักส่งมา และส่งคำสั่งกลับไป
  type Np = { has: boolean; title: string; artist: string; playing: boolean; pos: number; dur: number; tint: string; cover: string | null; theme?: Record<string, string>; lang?: 'en' | 'th' };
  let s = $state<Np>({ has: false, title: '', artist: '', playing: false, pos: 0, dur: 0, tint: '#1b2a3a', cover: null });
  const cmd = (c: string, v?: number) => emit('np-cmd', { cmd: c, v });

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

<div class="mini" style:--tint={s.tint}>
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
  </div>
</div>

<style>
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
