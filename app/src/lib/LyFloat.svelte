<script lang="ts">
  import { onMount } from 'svelte';
  import { listen, emit } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { i18n, tr } from '$lib/i18n.svelte';

  // หน้าต่างเนื้อเพลงลอย: โปร่งใส อยู่บนสุด ลากย้ายได้ แสดงบรรทัดปัจจุบันตัวใหญ่ + บรรทัดถัดไป
  type L = { cur: string; next: string; title: string; accent?: string; lang?: 'en' | 'th' };
  let s = $state<L>({ cur: '', next: '', title: '' });
  let big = $state(true);
  onMount(() => {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
    const un = listen<L>('lyfloat', (e) => {
      s = e.payload;
      if (s.lang) i18n.lang = s.lang;
    });
    emit('lyfloat-ready');
    return () => void un.then((f) => f());
  });
  const win = getCurrentWindow();
</script>

<div class="lf" class:big data-tauri-drag-region style:--acc={s.accent ?? '#5eead4'}>
  <div class="bar" data-tauri-drag-region>
    <small data-tauri-drag-region>{s.title}</small>
    <button aria-label={tr('สลับขนาดตัวอักษร')} onclick={() => (big = !big)}>A</button>
    <button aria-label={tr('ปิดหน้าต่างเนื้อเพลง')} onclick={() => win.close()}>×</button>
  </div>
  <p class="cur" data-tauri-drag-region>{s.cur || (s.title ? '♪' : tr('ยังไม่มีเนื้อเพลงที่มีเวลา'))}</p>
  <p class="nx" data-tauri-drag-region>{s.next}</p>
</div>

<style>
  :global(html, body) { margin: 0; background: transparent !important; overflow: hidden; }
  .lf { height: 100vh; box-sizing: border-box; padding: 6px 16px 10px; display: flex; flex-direction: column; justify-content: center; gap: 4px; color: #fff; text-align: center; border-radius: 14px; background: rgba(0, 0, 0, 0.35); cursor: move; user-select: none; font-family: system-ui, 'Noto Sans Thai', sans-serif; }
  .lf:not(:hover) .bar { opacity: 0; }
  .bar { position: absolute; top: 4px; left: 10px; right: 6px; display: flex; align-items: center; gap: 4px; transition: opacity 150ms; }
  .bar small { flex: 1; text-align: left; opacity: 0.7; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .bar button { all: unset; cursor: pointer; width: 22px; height: 22px; text-align: center; border-radius: 6px; font-weight: 700; }
  .bar button:hover { background: rgba(255, 255, 255, 0.2); }
  .cur { margin: 0; font-weight: 800; font-size: 1.4rem; color: var(--acc); text-shadow: 0 2px 6px rgba(0, 0, 0, 0.8); overflow-wrap: anywhere; }
  .big .cur { font-size: 2rem; }
  .nx { margin: 0; font-size: 1rem; opacity: 0.75; text-shadow: 0 1px 4px rgba(0, 0, 0, 0.8); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
