<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen, emit } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { i18n, tr, type Lang } from '$lib/i18n.svelte';
  import Icon from '$lib/Icon.svelte';
  import {
    fmt, fmtDate, cover, art, hue, parseLrc, lrcIndex, linkKey, isPlaylistLink, isSupportedLink, versionAgeDays,
    type Track, type Playlist, type LrcLine
  } from '$lib/data';
  import { THEMES, LAYOUTS, DEFAULT_LOOK, normalizeLook, lookVars, isLight, type Look } from '$lib/look';

  type Done = { title: string; artist: string; album: string; dur: number; playlist: string | null; url: string; thumb: string | null; path: string; cover: string | null };

  // ---------- state ----------
  let library = $state<Track[]>([]);
  let playlists = $state<Playlist[]>([]);
  let loaded = $state(false); // กันเขียนทับไฟล์ด้วยคลังว่างก่อนอ่านเสร็จ
  let view = $state('home'); // home | songs | download | tools | pl:<id> | auto:<id>
  let query = $state('');
  let current = $state<Track | null>(null);
  let srcIds = $state<number[] | null>(null); // รายการที่กดเล่นมา (null = ทั้งคลัง)
  let playing = $state(false);
  let pos = $state(0);
  let resumePos = $state(0); // บันทึกทุก ~5 วินาที ไว้เล่นต่อ
  let shuffle = $state(false);
  let repeat = $state(false);
  let volume = $state(70);
  let panel = $state<'queue' | 'now' | 'lyrics' | null>('queue');
  let compact = $state(false);
  let selected = $state<number | null>(null);
  let notice = $state('');
  let missing = $state<number[]>([]);
  let audio = $state<HTMLAudioElement>();
  let pendingSeek = 0;
  let look = $state<Look>({ ...DEFAULT_LOOK });
  let speed = $state(1);
  let discord = $state(true); // แสดงเพลงที่ฟังใน Discord
  let discordMsg = $state('');
  let seeks = $state(0); // นับครั้งที่กรอเพลง เพื่อให้ Discord ปรับแถบเวลา

  // ---------- derived ----------
  const shown = $derived(
    library.filter((t) => `${t.title} ${tr(t.artist)} ${t.album}`.toLowerCase().includes(query.toLowerCase()))
  );
  const byAdded = $derived([...library].sort((a, b) => b.added.localeCompare(a.added)).slice(0, 6));
  const byPlays = $derived([...library].filter((t) => t.plays > 0).sort((a, b) => b.plays - a.plays).slice(0, 6));
  const byLast = $derived([...library].filter((t) => t.plays > 0).sort((a, b) => a.last.localeCompare(b.last)).slice(0, 6));
  const pl = $derived(view.startsWith('pl:') ? playlists.find((p) => p.id === +view.slice(3)) : undefined);
  const byIds = (ids: number[]) => ids.flatMap((id) => library.find((t) => t.id === id) ?? []);
  const plTracks = $derived(pl ? byIds(pl.ids) : []);

  // เพลย์ลิสต์อัตโนมัติ: คำนวณจากประวัติการฟัง ไม่มีข้อมูลซ้ำซ้อน
  const AUTO = [
    { id: 'fav', name: 'เพลงที่ถูกใจ' },
    { id: 'top', name: 'ให้ 4 ดาวขึ้นไป' },
    { id: 'freq', name: 'ฟังบ่อย' },
    { id: 'old', name: 'ไม่ได้ฟังนาน' },
    { id: 'new', name: 'เพิ่งโหลด 7 วัน' }
  ];
  const auto = $derived(view.startsWith('auto:') ? AUTO.find((a) => a.id === view.slice(5)) : undefined);
  const autoTracks = $derived.by(() => {
    if (!auto) return [];
    if (auto.id === 'fav') return library.filter((t) => t.fav);
    if (auto.id === 'top') return library.filter((t) => (t.rating ?? 0) >= 4).sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0));
    if (auto.id === 'freq') return library.filter((t) => t.plays > 0).sort((a, b) => b.plays - a.plays).slice(0, 50);
    if (auto.id === 'old') return [...library].sort((a, b) => a.last.localeCompare(b.last)).slice(0, 50);
    const since = new Date(Date.now() - 7 * 864e5).toISOString().slice(0, 10);
    return library.filter((t) => t.added >= since).sort((a, b) => b.added.localeCompare(a.added));
  });

  const srcTracks = $derived(srcIds ? byIds(srcIds) : library);
  const queue = $derived.by(() => {
    const i = current ? srcTracks.findIndex((t) => t.id === current!.id) : -1;
    return i < 0 ? [] : srcTracks.slice(i + 1).concat(srcTracks.slice(0, i));
  });

  // ---------- โหลด/บันทึกคลัง ----------
  onMount(async () => {
    try {
      const raw = await invoke<string>('read_library');
      if (raw) {
        const d = JSON.parse(raw);
        library = d.tracks ?? [];
        playlists = d.playlists ?? [];
        const p = d.prefs ?? {};
        volume = p.volume ?? volume;
        shuffle = !!p.shuffle;
        repeat = !!p.repeat;
        discord = p.discord ?? true;
        speed = p.speed ?? 1;
        if (p.lang === 'th' || p.lang === 'en') i18n.lang = p.lang;
        look = normalizeLook(p.look);
        if (!look.startPanel) panel = null;
        const t = library.find((x) => x.id === p.resumeId);
        if (t && audio) {
          current = t; // เล่นต่อจากที่ค้าง (ยังไม่เล่นจนกว่าจะกดเอง)
          pendingSeek = resumePos = pos = p.resumePos ?? 0;
          audio.src = convertFileSrc(t.path);
        }
      }
    } catch (e) {
      notice = tr('อ่านคลังเพลงไม่ได้: {0}', tr(String(e)));
      return; // ไม่เปิด loaded เพื่อไม่ให้เขียนทับไฟล์เดิมที่อาจยังดีอยู่
    }
    loaded = true;
    invoke<string[]>('missing_files', { paths: library.map((t) => t.path) })
      .then((m) => (missing = library.filter((t) => m.includes(t.path)).map((t) => t.id)))
      .catch(() => {});
    refreshTools();
    await listen<{ id: number; pct: number; title: string | null }>('dl-progress', (e) => {
      const d = downloads.find((x) => x.id === e.payload.id);
      if (!d) return;
      if (e.payload.title && !d.playlist) d.title = e.payload.title;
      if (e.payload.pct > 0) d.pct = e.payload.pct;
      d.state = e.payload.pct >= 100 ? 'converting' : 'downloading';
    });
    await listen<[number, Done]>('dl-done', (e) => onTrackDone(...e.payload));
    await listen<{ cmd: string; v?: number }>('np-cmd', (e) => {
      const { cmd, v } = e.payload;
      if (cmd === 'toggle') toggle();
      else if (cmd === 'next') next();
      else if (cmd === 'prev') prev();
      else if (cmd === 'seek' && audio && v !== undefined) audio.currentTime = v;
    });
    await listen('np-ready', () => pushNp());
  });
  // shortcut: เขียนทั้งไฟล์ทุกครั้งที่แก้ (หน่วง 400ms), พอคลังเกินหลักพันเพลงค่อยย้ายไป SQLite
  $effect(() => {
    if (!loaded) return;
    const json = JSON.stringify({
      tracks: $state.snapshot(library),
      playlists: $state.snapshot(playlists),
      prefs: { volume, shuffle, repeat, discord, speed, lang: i18n.lang, look: $state.snapshot(look), resumeId: current?.id ?? null, resumePos }
    });
    const timer = setTimeout(() => invoke('write_library', { json }).catch((e) => (notice = tr('บันทึกไม่ได้: {0}', tr(String(e))))), 400);
    return () => clearTimeout(timer);
  });

  // ---------- player ----------
  function play(t: Track, list?: Track[]) {
    if (!audio) return;
    if (missing.includes(t.id)) {
      notice = tr('ไฟล์ของ “{0}” หายไป ลองลบออกจากคลังแล้วโหลดใหม่', t.title);
      return;
    }
    srcIds = list ? list.map((x) => x.id) : null;
    current = t;
    pos = resumePos = 0;
    notice = '';
    audio.src = convertFileSrc(t.path);
    audio.play().catch(() => {});
    t.plays += 1;
    t.last = new Date().toISOString().slice(0, 10);
  }
  function toggle() {
    if (!audio || !current) return;
    if (audio.paused) audio.play().catch(() => {});
    else audio.pause();
  }
  function next() {
    if (!current) return;
    const others = srcTracks.filter((t) => t.id !== current!.id);
    const n = shuffle ? others[Math.floor(Math.random() * others.length)] : queue[0];
    if (n) play(n, srcIds ? srcTracks : undefined);
    else playing = false;
  }
  function prev() {
    if (!current || !audio) return;
    if (audio.currentTime > 3) audio.currentTime = 0;
    else play(queue[queue.length - 1] ?? current, srcIds ? srcTracks : undefined);
  }
  function ended() {
    if (repeat && current) return play(current, srcIds ? srcTracks : undefined);
    next();
  }
  function onTime() {
    if (!audio) return;
    pos = audio.currentTime;
    if (Math.floor(pos / 5) !== Math.floor(resumePos / 5)) resumePos = pos;
  }
  function onMeta() {
    if (audio && pendingSeek) audio.currentTime = pendingSeek;
    pendingSeek = 0;
  }
  $effect(() => {
    if (!audio) return;
    audio.defaultPlaybackRate = speed; // ตั้ง default ด้วย เพราะเปลี่ยนเพลงแล้วเบราว์เซอร์จะรีเซ็ตกลับเป็นค่านี้
    audio.playbackRate = speed;
  });
  const SPEEDS = [0.75, 1, 1.25, 1.5, 2];
  const cycleSpeed = () => (speed = SPEEDS[(SPEEDS.indexOf(speed) + 1) % SPEEDS.length]);

  // ---------- ตัวแสดงคลื่นเสียง: ฟังเสียงผ่าน captureStream (ไม่ต่อเข้าทางออกเสียง จึงไม่มีทางทำให้เพลงเงียบ) ----------
  let vizCanvas = $state<HTMLCanvasElement>();
  let actx: AudioContext | undefined;
  let analyser: AnalyserNode | undefined;
  let vsrc: MediaStreamAudioSourceNode | undefined;
  let vizSrc = '';
  function attachViz() {
    if (!audio || !look.viz || vizSrc === audio.currentSrc) return;
    try {
      actx ??= new AudioContext();
      analyser ??= Object.assign(actx.createAnalyser(), { fftSize: 128, smoothingTimeConstant: 0.8 });
      if (actx.state === 'suspended') actx.resume();
      vsrc?.disconnect();
      const stream = (audio as HTMLAudioElement & { captureStream(): MediaStream }).captureStream();
      if (!stream.getAudioTracks().length) return;
      vsrc = actx.createMediaStreamSource(stream);
      vsrc.connect(analyser);
      vizSrc = audio.currentSrc;
    } catch {
      /* เบราว์เซอร์ไม่รองรับ: แค่ไม่มีคลื่นเสียง เพลงยังเล่นปกติ */
    }
  }
  $effect(() => {
    const c = vizCanvas;
    if (!c || !playing || !look.viz || panel !== 'now' || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const g = c.getContext('2d')!;
    const data = new Uint8Array(64);
    let raf = 0;
    const draw = () => {
      analyser?.getByteFrequencyData(data);
      g.clearRect(0, 0, c.width, c.height);
      g.fillStyle = getComputedStyle(c).color;
      const n = 28, w = c.width / n;
      for (let i = 0; i < n; i++) {
        const h = Math.max(3, (data[Math.floor(i * 1.5)] / 255) * c.height);
        g.fillRect(i * w + 1, c.height - h, w - 3, h);
      }
      raf = requestAnimationFrame(draw);
    };
    draw();
    return () => cancelAnimationFrame(raf);
  });
  // ถ้า WebView ไม่ยอมให้อ่านไฟล์แบบ CORS: ปิดคลื่นเสียง แล้วโหลดเพลงเดิมต่อโดยไม่ใช้ CORS
  function audioError() {
    if (audio?.crossOrigin && current) {
      const at = audio.currentTime;
      audio.removeAttribute('crossorigin');
      look.viz = false;
      pendingSeek = at;
      audio.src = convertFileSrc(current.path);
      audio.play().catch(() => {});
      return;
    }
    playing = false;
    notice = tr('เล่นไฟล์นี้ไม่ได้ ไฟล์อาจถูกย้ายหรือลบ');
  }

  let fading = false;
  $effect(() => {
    if (audio && !fading) audio.volume = volume / 100;
  });

  // Space = เล่น/หยุด ยกเว้นตอนพิมพ์ข้อความหรืออยู่ในหน้าต่างย่อย; กันปุ่มที่โฟกัสอยู่ถูกกดซ้ำ
  const typing = (e: KeyboardEvent) => {
    const el = e.target as HTMLInputElement;
    return !!el.closest?.('dialog') || el.tagName === 'INPUT' || el.tagName === 'SELECT';
  };
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') ctx = null;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      return openPalette();
    }
    if (e.code !== 'Space' || (typing(e) && (e.target as HTMLInputElement).type !== 'range')) return;
    e.preventDefault();
    toggle();
  }
  function onKeyUp(e: KeyboardEvent) {
    if (e.code === 'Space' && !typing(e)) e.preventDefault();
  }

  // ---------- Media Session: ปุ่มมีเดียบนคีย์บอร์ด/หูฟัง + แผงมีเดียของ Windows ----------
  $effect(() => {
    if (!('mediaSession' in navigator)) return;
    const t = current;
    navigator.mediaSession.metadata = t
      ? new MediaMetadata({ title: t.title, artist: t.artist, album: t.album, artwork: t.cover ? [{ src: convertFileSrc(t.cover) }] : [] })
      : null;
  });
  $effect(() => {
    if ('mediaSession' in navigator) navigator.mediaSession.playbackState = playing ? 'playing' : 'paused';
  });
  onMount(() => {
    if (!('mediaSession' in navigator)) return;
    const h: [MediaSessionAction, MediaSessionActionHandler][] = [
      ['play', toggle], ['pause', toggle], ['previoustrack', prev], ['nexttrack', next],
      ['seekto', (d) => audio && d.seekTime !== undefined && (audio.currentTime = d.seekTime)]
    ];
    for (const [a, f] of h) navigator.mediaSession.setActionHandler(a, f);
  });


  // ---------- Discord Rich Presence: ส่งเฉพาะตอนเปลี่ยนเพลง/เล่น-หยุด/กรอ ไม่ส่งทุกวินาที ----------
  $effect(() => {
    if (!loaded) return;
    const t = current, on = discord, isPlaying = playing;
    void seeks;
    if (!on || !t) {
      invoke('discord_clear').catch(() => {});
      discordMsg = '';
      return;
    }
    const p = audio?.currentTime ?? 0;
    invoke('discord_update', { title: t.title, artist: tr(t.artist), playing: isPlaying, pos: p, dur: t.dur, cover: t.coverUrl ?? null })
      .then(() => (discordMsg = tr('เชื่อมต่อ Discord แล้ว')))
      .catch((e) => (discordMsg = tr(String(e))));
  });

  // ปุ่มทดสอบ: ส่งสถานะตัวอย่างไป Discord 8 วินาที แล้วกลับไปแสดงเพลงจริง (หรือล้างถ้าไม่ได้เล่นอะไร)
  async function testDiscord() {
    discord = true;
    discordMsg = tr('กำลังเชื่อมต่อ…');
    try {
      await invoke('discord_update', { title: tr('ทดสอบ MySound'), artist: tr('เชื่อมต่อสำเร็จ'), playing: true, pos: 0, dur: 8, cover: null });
      discordMsg = tr('เชื่อมต่อสำเร็จ ลองดูโปรไฟล์ของคุณใน Discord (8 วินาที)');
      setTimeout(() => seeks++, 8000);
    } catch (e) {
      discordMsg = tr(String(e));
    }
  }

  // ---------- หน้าตา: แปลงค่าที่เลือกเป็นตัวแปร CSS ----------
  const themeVars = $derived(lookVars(look));
  $effect(() => void (document.documentElement.lang = i18n.lang));
  $effect(() => {
    const r = document.documentElement;
    for (const [k, v] of Object.entries(themeVars)) r.style.setProperty(k, v);
    r.style.fontSize = `${look.font}%`;
    r.style.colorScheme = isLight(look) ? 'light' : 'dark';
  });
  const pickTheme = (t: (typeof THEMES)[number]) => (look = { ...look, accent: t.accent, base: t.base, raised: t.raised, outer: t.outer });
  const pickLayout = (l: (typeof LAYOUTS)[number]) => (look = { ...look, ...l.v });
  // รูปพื้นหลัง: คัดลอกเข้าโฟลเดอร์ของแอป (asset protocol อ่านได้เฉพาะที่นั่น) แล้วลบรูปเก่า
  let bgUrl = $state('');
  let bgBusy = $state(false);
  let bgMsg = $state('');
  async function setBg(source: string) {
    bgBusy = true;
    bgMsg = '';
    try {
      const r = await invoke<{ path: string }>('import_cover', { key: 'bg', source, old: look.bg || null });
      look.bg = r.path;
      bgUrl = '';
    } catch (e) {
      bgMsg = tr(String(e));
    }
    bgBusy = false;
  }
  async function pickBg() {
    const f = await open({ filters: [{ name: tr('รูปภาพ'), extensions: ['jpg', 'jpeg', 'png', 'webp', 'gif'] }] });
    if (typeof f === 'string') await setBg(f);
  }
  const layoutIs = (l: (typeof LAYOUTS)[number]) => (Object.keys(l.v) as (keyof typeof l.v)[]).every((k) => look[k] === l.v[k]);
  const sameTheme = (t: (typeof THEMES)[number]) => t.accent === look.accent && t.base === look.base && t.raised === look.raised && t.outer === look.outer;

  // ---------- สีธีมตามปก ----------
  let tint = $state('hsl(200 40% 30%)');
  $effect(() => {
    const t = current;
    if (!t) return;
    const fallback = `hsl(${hue(t.title)} 45% 32%)`;
    if (!t.cover) return void (tint = fallback);
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      try {
        const c = document.createElement('canvas');
        c.width = c.height = 8;
        const x = c.getContext('2d')!;
        x.drawImage(img, 0, 0, 8, 8);
        const d = x.getImageData(0, 0, 8, 8).data;
        let r = 0, g = 0, b = 0;
        for (let i = 0; i < d.length; i += 4) (r += d[i]), (g += d[i + 1]), (b += d[i + 2]);
        tint = `rgb(${r / 64 | 0} ${g / 64 | 0} ${b / 64 | 0})`;
      } catch {
        tint = fallback; // ถ้า canvas อ่านภาพไม่ได้ (CORS) ใช้สีจากชื่อแทน
      }
    };
    img.onerror = () => (tint = fallback);
    img.src = convertFileSrc(t.cover);
  });

  // ---------- ตั้งเวลาปิดเสียง ----------
  const SLEEP = [0, 15, 30, 60];
  let sleepMin = $state(0);
  let sleepEnd = $state(0);
  let sleepLeft = $state(0);
  function cycleSleep() {
    sleepMin = SLEEP[(SLEEP.indexOf(sleepMin) + 1) % SLEEP.length];
    sleepEnd = sleepMin ? Date.now() + sleepMin * 60000 : 0;
    sleepLeft = sleepMin * 60;
  }
  $effect(() => {
    if (!sleepEnd) return;
    const timer = setInterval(() => {
      sleepLeft = Math.max(0, Math.ceil((sleepEnd - Date.now()) / 1000));
      if (!audio) return;
      fading = sleepLeft <= 10;
      if (fading) audio.volume = (volume / 100) * (sleepLeft / 10); // ค่อยๆ เบาใน 10 วินาทีสุดท้าย
      if (sleepLeft <= 0) {
        audio.pause();
        fading = false;
        audio.volume = volume / 100;
        sleepEnd = sleepMin = 0;
      }
    }, 500);
    return () => {
      clearInterval(timer);
      fading = false;
      if (audio) audio.volume = volume / 100;
    };
  });

  // ---------- มินิเพลเยอร์ (หน้าต่างแยก คุยกันผ่าน event) ----------
  const npState = () => ({
    theme: themeVars,
    lang: i18n.lang,
    has: !!current, title: current?.title ?? '', artist: current?.artist ?? '', playing, pos, dur: current?.dur ?? 0, tint,
    cover: current?.cover ? convertFileSrc(current.cover) : null
  });
  const pushNp = () => emit('np', npState()).catch(() => {});
  $effect(() => {
    pushNp();
  });
  async function openMini() {
    try {
      const ex = await WebviewWindow.getByLabel('mini');
      if (ex) return void (await ex.setFocus());
      new WebviewWindow('mini', { url: '/', title: 'MySound', width: 360, height: 160, resizable: false, alwaysOnTop: true });
    } catch (e) {
      notice = tr('เปิดมินิเพลเยอร์ไม่ได้: {0}', tr(String(e)));
    }
  }

  // ---------- เนื้อเพลง ----------
  type Ly = { status: 'loading' | 'none' | 'ready' | 'error'; lines: LrcLine[]; plain: string; msg?: string };
  const lyCache = new Map<number, Ly>();
  let ly = $state<Ly>({ status: 'none', lines: [], plain: '' });
  async function loadLyrics(t: Track, force = false) {
    if (t.lrc) {
      const lines = parseLrc(t.lrc);
      return void (ly = { status: 'ready', lines, plain: lines.length ? '' : t.lrc });
    }
    const hit = lyCache.get(t.id);
    if (hit && !force) return void (ly = hit);
    ly = { status: 'loading', lines: [], plain: '' };
    let r: Ly;
    try {
      const d = await invoke<{ synced: string | null; plain: string | null }>('lyrics', { title: t.title, artist: t.artist, dur: t.dur });
      const lines = d.synced ? parseLrc(d.synced) : [];
      r = lines.length || d.plain ? { status: 'ready', lines, plain: d.plain ?? '' } : { status: 'none', lines: [], plain: '' };
      lyCache.set(t.id, r);
    } catch (e) {
      r = { status: 'error', lines: [], plain: '', msg: tr(String(e)) };
    }
    if (current?.id === t.id) ly = r;
  }
  $effect(() => {
    if (panel === 'lyrics' && current) loadLyrics(current);
  });
  const lyIdx = $derived(ly.lines.length ? lrcIndex(ly.lines, pos) : -1);
  $effect(() => {
    if (lyIdx < 0 || panel !== 'lyrics') return;
    const smooth = !matchMedia('(prefers-reduced-motion: reduce)').matches;
    tick().then(() => document.querySelector('.ly .on')?.scrollIntoView({ block: 'center', behavior: smooth ? 'smooth' : 'auto' }));
  });

  let lyDlg = $state<HTMLDialogElement>();
  let lyText = $state('');
  let lyMsg = $state('');
  function openLyEdit() {
    if (!current) return;
    lyText = current.lrc ?? '';
    lyMsg = '';
    lyDlg?.showModal();
  }
  async function pickLrc() {
    const f = await open({ filters: [{ name: tr('เนื้อเพลง'), extensions: ['lrc', 'txt'] }] });
    if (typeof f !== 'string') return;
    try {
      lyText = await invoke<string>('read_text_file', { path: f });
      lyMsg = '';
    } catch (e) {
      lyMsg = tr(String(e));
    }
  }
  function saveLy(clear = false) {
    if (!current) return;
    const t = clear ? '' : lyText.trim();
    current.lrc = t || undefined;
    lyCache.delete(current.id);
    loadLyrics(current, true);
    lyDlg?.close();
  }

  // ---------- playlist ----------
  let renaming = $state(false);
  let renameVal = $state('');
  function newPlaylist(name?: string) {
    const id = Date.now() + Math.floor(Math.random() * 1000);
    playlists.push({ id, name: name ?? tr('เพลย์ลิสต์ใหม่ {0}', playlists.length + 1), ids: [] });
    return id;
  }
  function addToPlaylist(pid: number, tid: number) {
    const p = playlists.find((x) => x.id === pid);
    if (p && !p.ids.includes(tid)) p.ids.push(tid);
  }
  function onPick(e: Event) {
    const sel = e.target as HTMLSelectElement;
    if (selected !== null && sel.value) addToPlaylist(+sel.value, selected);
    sel.value = '';
  }
  function startRename() {
    renameVal = pl?.name ?? '';
    renaming = true;
  }
  function endRename() {
    if (pl && renameVal.trim()) pl.name = renameVal.trim();
    renaming = false;
  }
  function move(pid: number, tid: number, d: -1 | 1) {
    const p = playlists.find((x) => x.id === pid);
    if (!p) return;
    const i = p.ids.indexOf(tid), j = i + d;
    if (i < 0 || j < 0 || j >= p.ids.length) return;
    [p.ids[i], p.ids[j]] = [p.ids[j], p.ids[i]];
  }

  // ---------- ถูกใจ / ให้ดาว ----------
  const toggleFav = (t: Track) => (t.fav = !t.fav);

  // ---------- ลบ (มีหน้าต่างยืนยันเสมอ) ----------
  let confirmDlg = $state<HTMLDialogElement>();
  let confirmS = $state<{ msg: string; label: string; ok: () => void }>({ msg: '', label: '', ok: () => {} });
  function ask(msg: string, label: string, ok: () => void) {
    confirmS = { msg, label, ok };
    confirmDlg?.showModal();
  }
  async function removeTracks(ts: Track[]) {
    const ids = new Set(ts.map((t) => t.id));
    if (current && ids.has(current.id)) {
      audio?.pause();
      audio?.removeAttribute('src');
      current = null;
    }
    await invoke('delete_files', { paths: ts.flatMap((t) => [t.path, ...(t.cover ? [t.cover] : [])]) }).catch(() => {});
    library = library.filter((t) => !ids.has(t.id));
    for (const p of playlists) p.ids = p.ids.filter((i) => !ids.has(i));
    missing = missing.filter((i) => !ids.has(i));
    selected = null;
  }
  const askDeleteTrack = (t: Track) =>
    ask(tr('ลบ “{0}” ออกจากคลัง? ไฟล์เสียงและปกจะถูกลบจากเครื่อง ย้อนกลับไม่ได้', t.title), tr('ลบเพลง'), () => removeTracks([t]));
  const askDeletePlaylist = () =>
    pl && ask(tr('ลบเพลย์ลิสต์ “{0}”? (เพลงในคลังยังอยู่ครบ)', pl.name), tr('ลบเพลย์ลิสต์'), () => {
      playlists = playlists.filter((p) => p.id !== pl!.id);
      view = 'home';
    });

  // ---------- คลิกขวา ----------
  type Ctx = { x: number; y: number; t: Track; plId?: number };
  let ctx = $state<Ctx | null>(null);
  function menu(e: MouseEvent, t: Track, plId?: number) {
    e.preventDefault();
    const h = 40 + 34 * (4 + playlists.length + (plId ? 3 : 0));
    ctx = { x: Math.min(e.clientX, innerWidth - 230), y: Math.min(e.clientY, innerHeight - h), t, plId };
  }
  const act = (f: () => void) => () => (f(), (ctx = null));

  // ---------- ตั้งค่าเพลง ----------
  let dlg = $state<HTMLDialogElement>();
  let edit = $state({ id: 0, title: '', artist: '', album: '', src: '' });
  let editMsg = $state('');
  let editOk = $state(false);
  let busy = $state(false);
  const editing = $derived(library.find((t) => t.id === edit.id));
  function openEdit(t: Track) {
    ctx = null;
    edit = { id: t.id, title: t.title, artist: t.artist, album: t.album, src: '' };
    editMsg = '';
    dlg?.showModal();
  }
  async function run(job: () => Promise<string | void>) {
    busy = true;
    editMsg = '';
    try {
      const ok = await job();
      editOk = !!ok;
      if (ok) editMsg = ok;
    } catch (e) {
      editOk = false;
      editMsg = tr(String(e));
    } finally {
      busy = false;
    }
  }
  const findGenius = () =>
    run(async () => {
      if (!editing) return;
      const r = await invoke<{ path: string; url: string | null }>('genius_cover', { key: String(editing.id), query: `${edit.artist} ${edit.title}`.trim(), old: editing.cover ?? null });
      editing.cover = r.path;
      editing.coverUrl = r.url;
      seeks++; // ให้ Discord ได้ปกใหม่
    });
  const useSource = () =>
    run(async () => {
      if (!editing || !edit.src.trim()) return;
      const r = await invoke<{ path: string; url: string | null }>('import_cover', { key: String(editing.id), source: edit.src, old: editing.cover ?? null });
      editing.cover = r.path;
      editing.coverUrl = r.url;
      seeks++;
      edit.src = '';
    });
  async function pickCover() {
    const f = await open({ filters: [{ name: tr('รูปภาพ'), extensions: ['jpg', 'jpeg', 'png', 'webp'] }] });
    if (typeof f === 'string') {
      edit.src = f;
      await useSource();
    }
  }
  const cleanAudio = () =>
    run(async () => {
      if (!editing) return;
      const wasCurrent = current?.id === editing.id;
      if (wasCurrent && audio) (audio.pause(), audio.removeAttribute('src'), audio.load()); // Windows ล็อกไฟล์ที่กำลังเล่นอยู่
      const r = await invoke<{ path: string; dur: number }>('process_audio', { path: editing.path });
      editing.path = r.path;
      if (r.dur) editing.dur = r.dur;
      if (wasCurrent && audio) audio.src = convertFileSrc(r.path);
      return tr('ตัดความเงียบและปรับความดังเสร็จแล้ว');
    });
  function saveEdit() {
    if (editing && edit.title.trim()) {
      editing.title = edit.title.trim();
      editing.artist = edit.artist.trim() || editing.artist;
      editing.album = edit.album.trim() || editing.album;
      lyCache.delete(editing.id);
      if (current?.id === editing.id && panel === 'lyrics') loadLyrics(editing, true);
    }
    dlg?.close();
  }

  // ---------- ค้นหาด่วน (Ctrl+K) ----------
  let palDlg = $state<HTMLDialogElement>();
  let pal = $state({ q: '', i: 0 });
  type PalItem = { key: string; label: string; sub: string; run: () => void };
  const palItems = $derived.by((): PalItem[] => {
    const q = pal.q.trim().toLowerCase();
    const go = (v: string) => () => (view = v);
    const items: PalItem[] = [
      { key: 'c-home', label: tr('หน้าแรก'), sub: tr('ไปที่'), run: go('home') },
      { key: 'c-songs', label: tr('เพลงทั้งหมด'), sub: tr('ไปที่'), run: go('songs') },
      { key: 'c-dl', label: tr('ดาวน์โหลดเพลง'), sub: tr('ไปที่'), run: go('download') },
      { key: 'c-tools', label: tr('เครื่องมือ'), sub: tr('ไปที่'), run: go('tools') },
      { key: 'c-shuf', label: tr('สุ่มเล่นทั้งคลัง'), sub: tr('คำสั่ง'), run: () => library.length && ((shuffle = true), play(library[Math.floor(Math.random() * library.length)])) },
      ...AUTO.map((a) => ({ key: 'a' + a.id, label: tr(a.name), sub: tr('เพลย์ลิสต์อัตโนมัติ'), run: go('auto:' + a.id) })),
      ...playlists.map((p) => ({ key: 'p' + p.id, label: p.name, sub: tr('เพลย์ลิสต์'), run: go('pl:' + p.id) }))
    ].filter((x) => !q || x.label.toLowerCase().includes(q));
    const tracks = q
      ? library.filter((t) => `${t.title} ${tr(t.artist)} ${t.album}`.toLowerCase().includes(q)).slice(0, 8).map((t) => ({ key: 't' + t.id, label: t.title, sub: tr(t.artist), run: () => play(t) }))
      : [];
    return [...tracks, ...items].slice(0, 12);
  });
  function openPalette() {
    pal = { q: '', i: 0 };
    palDlg?.showModal();
  }
  function palKey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const n = palItems.length || 1;
      pal.i = (pal.i + (e.key === 'ArrowDown' ? 1 : n - 1)) % n;
    } else if (e.key === 'Enter') {
      e.preventDefault();
      palItems[pal.i]?.run();
      palDlg?.close();
    }
  }

  // ---------- download ----------
  type Dl = {
    id: number;
    url: string;
    src: Track['src'];
    title: string;
    pct: number;
    state: 'waiting' | 'downloading' | 'converting' | 'done' | 'failed';
    msg?: string;
    playlist: boolean;
    count: number;
  };
  let downloads = $state<Dl[]>([]);
  let url = $state('');
  let err = $state('');
  let urlInput = $state<HTMLInputElement>();
  let seq = 0;

  function onTrackDone(job: number, r: Done) {
    const d = downloads.find((x) => x.id === job);
    if (library.some((t) => t.path === r.path)) return; // ไฟล์เดิมอยู่แล้ว (เช่น เพลงในเพลย์ลิสต์ที่เคยโหลด)
    const today = new Date().toISOString().slice(0, 10);
    const id = Date.now() + seq++;
    library.unshift({ id, src: d?.src ?? 'youtube', added: today, last: today, plays: 0, title: r.title, artist: r.artist, album: r.album, dur: r.dur, path: r.path, cover: r.cover, coverUrl: r.thumb, url: r.url });
    if (d) {
      d.count++;
      if (!d.playlist) d.title = r.title;
    }
    if (r.playlist) addToPlaylist(playlists.find((p) => p.name === r.playlist)?.id ?? newPlaylist(r.playlist), id);
  }

  /** ตรวจลิงก์ + สร้างรายการในรายการดาวน์โหลด (ยังไม่เริ่มโหลด) ถ้าใช้ไม่ได้คืนข้อความบอกเหตุผล */
  function prepare(u: string): { error: string } | { d: Dl } {
    if (!isSupportedLink(u)) return { error: tr('ลิงก์ต้องเป็นของ YouTube หรือ SoundCloud ลองคัดลอกลิงก์จากแถบที่อยู่ของเบราว์เซอร์') };
    const isList = isPlaylistLink(u);
    const dup = !isList && library.find((t) => t.url && linkKey(t.url) === linkKey(u));
    if (dup) return { error: tr('มีเพลงนี้ในคลังแล้ว: “{0}”', dup.title) };
    downloads.unshift({ id: Date.now() + seq++, url: u, src: /soundcloud/.test(u) ? 'soundcloud' : 'youtube', title: isList ? tr('เพลย์ลิสต์') : u, pct: 0, state: 'waiting', playlist: isList, count: 0 });
    return { d: downloads[0] };
  }
  async function runJob(d: Dl) {
    d.state = 'waiting';
    d.msg = undefined;
    d.pct = 0;
    try {
      await invoke('download', { id: d.id, url: d.url, playlist: d.playlist });
      d.state = 'done';
      d.pct = 100;
    } catch (e) {
      d.state = 'failed';
      d.msg = tr(String(e));
    }
  }
  async function addDownload(raw?: string) {
    const r = prepare((raw ?? url).trim());
    if ('error' in r) {
      err = r.error;
      urlInput?.focus();
      return;
    }
    err = '';
    url = '';
    pasted = '';
    view = 'download';
    await runJob(r.d);
  }

  // วางหลายลิงก์ทีละบรรทัด: โหลดพร้อมกันครั้งละ 2 เพลง
  let bulk = $state('');
  let bulkMsg = $state('');
  async function bulkDownload() {
    const links = [...new Set(bulk.split(/\r?\n/).map((x) => x.trim()).filter(Boolean))];
    const jobs: Dl[] = [];
    const skipped: string[] = [];
    for (const l of links) {
      const r = prepare(l);
      if ('error' in r) skipped.push(r.error);
      else jobs.push(r.d);
    }
    bulk = '';
    bulkMsg = tr('เพิ่มในคิว {0} ลิงก์', jobs.length) + (skipped.length ? tr(' ข้าม {0} (ซ้ำหรือไม่ใช่ลิงก์ที่รองรับ)', skipped.length) : '');
    let i = 0;
    await Promise.all(Array.from({ length: 2 }, async () => { while (i < jobs.length) await runJob(jobs[i++]); }));
  }

  // ค้นเพลงในแอป
  type Hit = { title: string; uploader: string; dur: number; url: string; thumb: string | null };
  let sq = $state('');
  let sSource = $state<'youtube' | 'soundcloud'>('youtube');
  let hits = $state<Hit[]>([]);
  let sBusy = $state(false);
  let sErr = $state('');
  let searched = $state(false);
  async function doSearch() {
    const q = sq.trim();
    if (!q) return;
    sBusy = true;
    sErr = '';
    searched = true;
    try {
      hits = await invoke<Hit[]>('search_online', { query: q, source: sSource });
    } catch (e) {
      sErr = tr(String(e));
      hits = [];
    }
    sBusy = false;
  }
  const haveHit = (h: Hit) => library.some((t) => t.url && linkKey(t.url) === linkKey(h.url));
  const stateText = { waiting: 'รอคิว', downloading: 'กำลังโหลด', converting: 'กำลังบันทึกไฟล์', done: 'เสร็จแล้ว', failed: 'ไม่สำเร็จ' };

  // วางลิงก์ที่ไหนก็ได้ในแอป -> ถามว่าจะโหลดเลยไหม
  let pasted = $state('');
  let pasteTimer: ReturnType<typeof setTimeout>;
  function onPaste(e: ClipboardEvent) {
    const el = e.target as HTMLElement;
    if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') return;
    const t = e.clipboardData?.getData('text')?.trim() ?? '';
    if (!t || /\s/.test(t) || !isSupportedLink(t)) return;
    pasted = t;
    clearTimeout(pasteTimer);
    pasteTimer = setTimeout(() => (pasted = ''), 15000);
  }

  // ---------- เครื่องมือ ----------
  let ytVer = $state<string | null | undefined>(undefined); // undefined = กำลังตรวจ, null = ไม่พบ
  let ffVer = $state<string | null | undefined>(undefined);
  const ytAge = $derived(ytVer ? versionAgeDays(ytVer) : null);
  let toolMsg = $state('');
  let toolBusy = $state(false);
  async function refreshTools() {
    ytVer = await invoke<string | null>('tool_version', { name: 'yt-dlp' }).catch(() => null);
    ffVer = await invoke<string | null>('tool_version', { name: 'ffmpeg' }).catch(() => null);
  }
  async function updateYtdlp() {
    toolBusy = true;
    toolMsg = tr('กำลังอัปเดต…');
    try {
      toolMsg = await invoke<string>('ytdlp_update');
    } catch (e) {
      toolMsg = tr(String(e));
    }
    toolBusy = false;
    refreshTools();
  }
  async function exportAll() {
    const dir = await open({ directory: true, title: tr('เลือกโฟลเดอร์ปลายทาง') });
    if (typeof dir !== 'string') return;
    toolBusy = true;
    toolMsg = tr('กำลังส่งออก…');
    try {
      const items = library.filter((t) => !missing.includes(t.id)).map((t) => ({ path: t.path, title: t.title, artist: t.artist, album: t.album, cover: t.cover ?? null }));
      const r = await invoke<{ done: number; tagged: number; failed: string[] }>('export_library', { dest: dir, items });
      toolMsg = tr('ส่งออก {0} เพลงไปที่ {1}', r.done, dir) + (ffVer ? tr(' (ฝังแท็กและปก {0} เพลง)', r.tagged) : tr(' (ไม่มี ffmpeg จึงคัดลอกไฟล์เฉยๆ ไม่ฝังแท็ก)')) + (r.failed.length ? tr(' ล้มเหลว {0} เพลง', r.failed.length) : '');
    } catch (e) {
      toolMsg = tr(String(e));
    }
    toolBusy = false;
  }
  const missingTracks = $derived(library.filter((t) => missing.includes(t.id)));
</script>

<svelte:window onkeydown={onKey} onkeyup={onKeyUp} onpaste={onPaste} onclick={() => (ctx = null)} oncontextmenu={(e) => e.defaultPrevented || (ctx = null)} />

<audio
  bind:this={audio}
  onplay={() => (playing = true)}
  onpause={() => (playing = false)}
  ontimeupdate={onTime}
  onloadedmetadata={onMeta}
  onseeked={() => seeks++}
  onended={ended}
  onerror={audioError}
  onplaying={attachViz}
  crossorigin="anonymous"
></audio>

{#snippet card(t: Track, list: Track[])}
  <button class="card" onclick={() => play(t, list)} oncontextmenu={(e) => menu(e, t)}>
    <div class="art" style:background={art(t)}>
      <span class="fab"><Icon n="play" size={18} /></span>
    </div>
    <b>{t.title}</b>
    <small>{tr(t.artist)}</small>
  </button>
{/snippet}

{#snippet rows(list: Track[], plId?: number)}
  <div class="thead"><span>#</span><span>{tr('ชื่อเพลง')}</span><span>{tr('อัลบั้ม')}</span><span>{tr('วันที่เพิ่ม')}</span><span></span><span class="r">{tr('เวลา')}</span><span></span></div>
  {#each list as t, i (t.id)}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="row"
      role="row"
      tabindex="0"
      class:sel={selected === t.id}
      class:now={current?.id === t.id}
      class:gone={missing.includes(t.id)}
      onclick={() => (selected = t.id)}
      ondblclick={() => play(t, list)}
      oncontextmenu={(e) => menu(e, t, plId)}
      onkeydown={(e) => e.target === e.currentTarget && e.key === 'Enter' && play(t, list)}
    >
      <span class="num"><i>{current?.id === t.id && playing ? '♪' : i + 1}</i><em><Icon n="play" size={14} /></em></span>
      <span class="title">
        <span class="thumb" style:background={art(t)}></span>
        <span><b>{t.title}</b><small>{missing.includes(t.id) ? tr('ไฟล์หาย · ') : ''}{tr(t.artist)}{#if t.rating} · {'★'.repeat(t.rating)}{/if}</small></span>
      </span>
      <span class="dim">{t.album}</span>
      <span class="dim">{fmtDate(t.added, i18n.lang)}</span>
      <button class="heart" class:on={t.fav} aria-label={t.fav ? tr('เลิกถูกใจ {0}', t.title) : tr('ถูกใจ {0}', t.title)} aria-pressed={!!t.fav} onclick={(e) => (e.stopPropagation(), toggleFav(t))} ondblclick={(e) => e.stopPropagation()}><Icon n="heart" size={16} fill={!!t.fav} /></button>
      <span class="dim r">{fmt(t.dur)}</span>
      <button class="more" aria-label={tr('ตัวเลือกเพิ่มเติมของ {0}', t.title)} aria-haspopup="menu" onclick={(e) => (e.stopPropagation(), menu(e, t, plId))} ondblclick={(e) => e.stopPropagation()}><Icon n="more" size={18} /></button>
    </div>
  {:else}
    <p class="empty">{library.length ? tr('ไม่พบเพลง') : tr('ยังไม่มีเพลง')}</p>
  {/each}
{/snippet}

<div class="app" class:compact class:noright={!panel} class:hasbg={!!look.bg} style:--tint={look.tint ? tint : look.base}>
  {#if look.bg}<div class="bgwrap" aria-hidden="true"><div class="bgimg" style:background-image="url('{convertFileSrc(look.bg)}')" style:filter="blur({look.bgBlur}px)"></div></div>{/if}
  <!-- แถบบน -->
  <header class="nav" data-tauri-drag-region>
    <button class="logo" translate="no" onclick={() => (view = 'home')}>MySound</button>
    <label class="search">
      <Icon n="search" size={18} />
      <input type="search" name="q" autocomplete="off" spellcheck="false" aria-label={tr('ค้นหาในคลังเพลง')} placeholder={tr('ค้นหาในคลังเพลง…  (Ctrl K = ค้นหาด่วน)')} bind:value={query} oninput={() => query && (view = 'songs')} />
    </label>
    <button class="pill lang" aria-label={tr('เปลี่ยนภาษา')} title={tr('เปลี่ยนภาษา')} onclick={() => (i18n.lang = i18n.lang === 'en' ? 'th' : 'en')}>{i18n.lang === 'en' ? 'ไทย' : 'EN'}</button>
    <button class="pill" onclick={() => (view = 'download')}><Icon n="download" size={16} /> {tr('ดาวน์โหลดเพลง')}</button>
    <div class="wc">
      <button aria-label={tr('ย่อหน้าต่าง')} onclick={() => getCurrentWindow().minimize()}><svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" stroke="currentColor" fill="none" /></svg></button>
      <button aria-label={tr('ขยายหรือคืนขนาดหน้าต่าง')} onclick={() => getCurrentWindow().toggleMaximize()}><svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="0.5" y="0.5" width="9" height="9" stroke="currentColor" fill="none" /></svg></button>
      <button class="x" aria-label={tr('ปิดหน้าต่าง')} onclick={() => getCurrentWindow().close()}><svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0l10 10M10 0L0 10" stroke="currentColor" fill="none" /></svg></button>
    </div>
  </header>

  <!-- ซ้าย: คลังเพลง -->
  <aside class="side panel">
    <div class="side-top">
      {#if !compact}<b>{tr('คลังเพลง')}</b>{/if}
      <button class="icon" title={tr('ย่อ/ขยาย')} aria-label={tr('ย่อหรือขยายคลังเพลง')} onclick={() => (compact = !compact)}><Icon n="sidebar" size={18} /></button>
      <button class="icon" title={tr('สร้างเพลย์ลิสต์')} aria-label={tr('สร้างเพลย์ลิสต์ใหม่')} onclick={() => (view = `pl:${newPlaylist()}`)}><Icon n="plus" size={18} /></button>
    </div>
    <nav>
      <button class="item" class:active={view === 'home'} onclick={() => (view = 'home')}><Icon n="home" /><span>{tr('หน้าแรก')}</span></button>
      <button class="item" class:active={view === 'songs'} onclick={() => (view = 'songs')}><Icon n="songs" /><span>{tr('เพลงทั้งหมด')}</span></button>
      <button class="item" class:active={view === 'download'} onclick={() => (view = 'download')}><Icon n="download" /><span>{tr('ดาวน์โหลด')}</span></button>
      <button class="item" class:active={view === 'tools'} onclick={() => (view = 'tools')}><Icon n="tools" /><span>{tr('เครื่องมือ')}</span></button>
      <button class="item" class:active={view === 'look'} onclick={() => (view = 'look')}><Icon n="palette" /><span>{tr('หน้าตา')}</span></button>
    </nav>
    <hr />
    <div class="pls">
      {#each AUTO as a (a.id)}
        <button class="item" class:active={view === `auto:${a.id}`} onclick={() => (view = `auto:${a.id}`)}>
          <span class="thumb auto" style:background={cover(a.name)}><Icon n="bolt" size={16} /></span>
          <span class="two"><b>{tr(a.name)}</b><small>{tr('อัตโนมัติ')}</small></span>
        </button>
      {/each}
      <hr />
      {#each playlists as p (p.id)}
        <button class="item" class:active={view === `pl:${p.id}`} onclick={() => (view = `pl:${p.id}`)}>
          <span class="thumb" style:background={cover(p.name)}></span>
          <span class="two"><b>{p.name}</b><small>{tr('{0} เพลง', p.ids.length)}</small></span>
        </button>
      {/each}
    </div>
  </aside>

  <!-- กลาง -->
  <main class="main panel">
    {#if view === 'home'}
      {#if !library.length}
        <p class="empty">{tr('ยังไม่มีเพลงในคลัง')} <button class="link" onclick={() => (view = 'download')}>{tr('ไปวางลิงก์ที่หน้าดาวน์โหลด')}</button></p>
      {:else}
        <h2>{tr('เพิ่งโหลด')}</h2>
        <div class="cards">{#each byAdded as t (t.id)}{@render card(t, byAdded)}{/each}</div>
        {#if byPlays.length}
          <h2>{tr('ฟังบ่อย')}</h2>
          <div class="cards">{#each byPlays as t (t.id)}{@render card(t, byPlays)}{/each}</div>
          <h2>{tr('ไม่ได้ฟังนาน')}</h2>
          <div class="cards">{#each byLast as t (t.id)}{@render card(t, byLast)}{/each}</div>
        {/if}
      {/if}
    {:else if view === 'songs'}
      <h1>{tr('เพลงทั้งหมด')}</h1>
      <p class="dim">{tr('{0} เพลง', shown.length)}</p>
      {#if selected !== null && playlists.length}
        <label class="addpl">{tr('เพิ่มเพลงที่เลือกใน')}
          <select onchange={onPick} aria-label={tr('เพิ่มเพลงที่เลือกลงเพลย์ลิสต์')}>
            <option value="">{tr('เลือกเพลย์ลิสต์…')}</option>
            {#each playlists as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
          </select>
        </label>
      {/if}
      {@render rows(shown)}
    {:else if view === 'download'}
      <h1>{tr('ดาวน์โหลดเพลง')}</h1>
      <p class="dim">{tr('วางลิงก์ YouTube หรือ SoundCloud — ลิงก์เพลย์ลิสต์ (youtube.com/playlist?list=… หรือ soundcloud.com/…/sets/…) จะโหลดทั้งชุดและสร้างเพลย์ลิสต์ให้')}</p>
      {#if ytVer === null}
        <p class="warn" role="alert">{tr('ไม่พบ yt-dlp ติดตั้งด้วย')} <code>winget install yt-dlp.yt-dlp</code> {tr('แล้วปิดและเปิดแอปใหม่')}</p>
      {:else if ytAge !== null && ytAge > 60}
        <p class="warn" role="alert">{tr('yt-dlp เวอร์ชันนี้เก่า {0} วันแล้ว ถ้าโหลดไม่ได้ ให้ไปหน้า', ytAge)} <button class="link" onclick={() => (view = 'tools')}>{tr('เครื่องมือ')}</button> {tr('แล้วกดอัปเดต')}</p>
      {/if}
      <form class="dlform" novalidate onsubmit={(e) => (e.preventDefault(), addDownload())}>
        <input
          bind:this={urlInput}
          type="url"
          name="url"
          inputmode="url"
          autocomplete="off"
          spellcheck="false"
          aria-label={tr('ลิงก์เพลง')}
          placeholder="https://www.youtube.com/watch?v=…"
          bind:value={url}
        />
        <button class="primary">{tr('ดาวน์โหลด')}</button>
      </form>
      <p class="err" role="status">{err}</p>
      <h2>{tr('ค้นหาเพลง')}</h2>
      <form class="dlform" role="search" onsubmit={(e) => (e.preventDefault(), doSearch())}>
        <div class="seg" role="group" aria-label={tr('แหล่งที่ค้น')}>
          <button type="button" class:on={sSource === 'youtube'} aria-pressed={sSource === 'youtube'} onclick={() => (sSource = 'youtube')}>YouTube</button>
          <button type="button" class:on={sSource === 'soundcloud'} aria-pressed={sSource === 'soundcloud'} onclick={() => (sSource = 'soundcloud')}>SoundCloud</button>
        </div>
        <input type="search" name="song" autocomplete="off" spellcheck="false" aria-label={tr('ชื่อเพลงที่จะค้น')} placeholder={tr('พิมพ์ชื่อเพลงหรือศิลปิน…')} bind:value={sq} />
        <button class="primary" disabled={sBusy}>{sBusy ? tr('กำลังค้นหา…') : tr('ค้นหา')}</button>
      </form>
      {#if sErr}<p class="errtxt" role="alert">{sErr}</p>{/if}
      {#if searched && !sBusy && !hits.length && !sErr}<p class="empty">{tr('ไม่พบผลลัพธ์')}</p>{/if}
      <div aria-live="polite">
        {#each hits as h (h.url)}
          <div class="dl hit">
            {#if h.thumb}<img class="thumb big" src={h.thumb} alt="" width="56" height="56" loading="lazy" />{:else}<span class="thumb big" style:background={cover(h.title)}></span>{/if}
            <div class="grow">
              <b>{h.title}</b>
              <small class="dim">{h.uploader}{h.dur ? ` · ${fmt(h.dur)}` : ''}</small>
            </div>
            <button class="ghost" disabled={haveHit(h)} onclick={() => addDownload(h.url)}>{haveHit(h) ? tr('มีในคลังแล้ว') : tr('ดาวน์โหลด')}</button>
          </div>
        {/each}
      </div>
      <details class="bulk">
        <summary>{tr('วางหลายลิงก์พร้อมกัน')}</summary>
        <textarea name="bulk" spellcheck="false" aria-label={tr('ลิงก์ทีละบรรทัด')} placeholder={tr('วางลิงก์ YouTube/SoundCloud ทีละบรรทัด…')} bind:value={bulk}></textarea>
        <button class="ghost" disabled={!bulk.trim()} onclick={bulkDownload}>{tr('เริ่มโหลดทั้งหมด (ครั้งละ 2 เพลง)')}</button>
        <p class="dim" role="status">{bulkMsg}</p>
      </details>
      <h2>{tr('รายการดาวน์โหลด')}</h2>
      {#if !downloads.length}<p class="empty">{tr('ยังไม่มีรายการดาวน์โหลด วางลิงก์ด้านบนเพื่อเริ่ม')}</p>{/if}
      <div aria-live="polite">
      {#each downloads as d (d.id)}
        <div class="dl">
          <span class="thumb big" style:background={cover(d.title)}></span>
          <div class="grow">
            <b>{d.playlist ? tr('เพลย์ลิสต์ · ได้แล้ว {0} เพลง', d.count) : d.title}</b>
            <small class="dim">{d.src === 'youtube' ? 'YouTube' : 'SoundCloud'} · {tr(stateText[d.state])}</small>
            {#if d.msg}<small class="errtxt">{d.msg}</small>{/if}
            <div class="bar"><i style:width="{d.pct}%"></i></div>
          </div>
          {#if d.state === 'failed'}<button class="ghost" onclick={() => runJob(d)}>{tr('ลองใหม่')}</button>{:else}<span class="dim num-tab">{Math.round(d.pct)}%</span>{/if}
        </div>
      {/each}
      </div>
    {:else if view === 'tools'}
      <h1>{tr('เครื่องมือ')}</h1>
      <h2>{tr('โปรแกรมที่แอปเรียกใช้')}</h2>
      <div class="tool">
        <div><b>yt-dlp</b><small class="dim">{ytVer === undefined ? tr('กำลังตรวจ…') : ytVer === null ? tr('ไม่พบ — winget install yt-dlp.yt-dlp') : tr('เวอร์ชัน {0}', ytVer) + (ytAge !== null ? tr(' ({0} วันที่แล้ว)', ytAge) : '')}</small></div>
        <button class="ghost" disabled={toolBusy} onclick={updateYtdlp}>{tr('อัปเดต yt-dlp')}</button>
      </div>
      <div class="tool">
        <div><b>ffmpeg</b><small class="dim">{ffVer === undefined ? tr('กำลังตรวจ…') : ffVer === null ? tr('ไม่พบ (ไม่บังคับ ใช้กับการปรับเสียงและฝังแท็กตอนส่งออก) — winget install Gyan.FFmpeg') : tr('เวอร์ชัน {0}', ffVer)}</small></div>
        <button class="ghost" onclick={refreshTools}>{tr('ตรวจใหม่')}</button>
      </div>
      <h2>Discord</h2>
      <div class="tool">
        <div><b>{tr('แสดงเพลงที่ฟังอยู่ในสถานะ Discord')}</b><small class="dim">{tr('ไม่ต้องเพิ่มแอปใน Discord เอง แค่เปิด Discord แบบแอปบนคอมไว้ แล้วกด “เชื่อมต่อ / ทดสอบ” (และเปิด Activity Privacy ในการตั้งค่า Discord)')}</small></div>
        <label class="switch"><input type="checkbox" name="discord" bind:checked={discord} /><span>{discord ? tr('เปิด') : tr('ปิด')}</span></label>
        <button class="ghost" onclick={testDiscord}>{tr('เชื่อมต่อ / ทดสอบ')}</button>
      </div>
      <p class="dim" role="status" aria-live="polite">{discord ? discordMsg : ''}</p>
      <h2>{tr('ส่งออกคลังเพลง')}</h2>
      <div class="tool">
        <div><b>{tr('คัดลอกทุกเพลงไปโฟลเดอร์อื่น')}</b><small class="dim">{tr('ตั้งชื่อไฟล์เป็น “ศิลปิน - ชื่อเพลง” ถ้ามี ffmpeg จะฝังชื่อ ศิลปิน อัลบั้ม และปกให้ (ไฟล์ m4a/mp3)')}</small></div>
        <button class="ghost" disabled={toolBusy || !library.length} onclick={exportAll}>{tr('เลือกโฟลเดอร์แล้วส่งออก…')}</button>
      </div>
      <p class="dim" role="status" aria-live="polite">{toolMsg}</p>
      <h2>{tr('ตรวจไฟล์เพลง')}</h2>
      {#if missingTracks.length}
        <p class="dim">{tr('พบเพลงที่ไฟล์เสียงหายไปจากเครื่อง {0} เพลง', missingTracks.length)}</p>
        <button class="ghost" onclick={() => ask(tr('เอา {0} เพลงที่ไฟล์หายออกจากคลัง?', missingTracks.length), tr('เอาออก'), () => removeTracks(missingTracks))}>{tr('เอาเพลงที่ไฟล์หายออกจากคลัง…')}</button>
      {:else}
        <p class="dim">{tr('ไฟล์เพลงทุกไฟล์อยู่ครบ')}</p>
      {/if}
    {:else if view === 'look'}
      <h1>{tr('หน้าตา')}</h1>
      <h2>{tr('ชุดสี')}</h2>
      <div class="themes">
        {#each THEMES as th (th.name)}
          <button class="theme" class:on={sameTheme(th)} aria-pressed={sameTheme(th)} onclick={() => pickTheme(th)}>
            <span class="sw" style:background={th.outer}><i style:background={th.base}></i><i style:background={th.raised}></i><i style:background={th.accent}></i></span>
            <b>{tr(th.name)}</b>
          </button>
        {/each}
      </div>
      <h2>{tr('ปรับสีเอง')}</h2>
      <div class="colors">
        <label>{tr('สีเน้น')}<input type="color" name="accent" bind:value={look.accent} /></label>
        <label>{tr('พื้นแผง')}<input type="color" name="base" bind:value={look.base} /></label>
        <label>{tr('พื้นการ์ด')}<input type="color" name="raised" bind:value={look.raised} /></label>
        <label>{tr('พื้นหลังนอกแผง')}<input type="color" name="outer" bind:value={look.outer} /></label>
      </div>
      <p class="dim">{tr('ตัวอักษรและเส้นขอบจะปรับสีเองตามความสว่างของพื้นแผง')}</p>
      <h2>{tr('รูปพื้นหลัง')}</h2>
      <div class="bgpick">
        <button class="ghost" disabled={bgBusy} onclick={pickBg}>{tr('เลือกรูป…')}</button>
        <input type="url" name="bgurl" autocomplete="off" spellcheck="false" aria-label={tr('ลิงก์รูปพื้นหลัง')} placeholder={tr('หรือวางลิงก์รูป…')} bind:value={bgUrl} />
        <button class="ghost" disabled={bgBusy || !bgUrl.trim()} onclick={() => setBg(bgUrl)}>{tr('ใช้รูปนี้')}</button>
        {#if look.bg}<button class="ghost danger" onclick={() => (look.bg = '')}>{tr('เอารูปออก')}</button>{/if}
      </div>
      <p class="errtxt" role="status">{bgMsg}</p>
      {#if look.bg}
        <div class="sliders">
          <label>{tr('ความทึบของแผง (ยิ่งต่ำยิ่งเห็นรูป)')} <output>{look.bgAlpha}%</output><input type="range" name="bgAlpha" min="15" max="100" step="5" bind:value={look.bgAlpha} /></label>
          <label>{tr('เบลอรูป')} <output>{look.bgBlur}px</output><input type="range" name="bgBlur" min="0" max="24" bind:value={look.bgBlur} /></label>
        </div>
      {/if}
      <h2>{tr('เลย์เอาต์')}</h2>
      <div class="themes">
        {#each LAYOUTS as l (l.name)}
          <button class="theme" class:on={layoutIs(l)} aria-pressed={layoutIs(l)} onclick={() => pickLayout(l)}>
            <b>{tr(l.name)}</b><small>{tr(l.hint)}</small>
          </button>
        {/each}
      </div>
      <div class="sliders">
        <label>{tr('ความกว้างแถบซ้าย')} <output>{look.sidebar}px</output><input type="range" name="sidebar" min="180" max="320" step="10" bind:value={look.sidebar} /></label>
        <label>{tr('ความกว้างแผงขวา')} <output>{look.right}px</output><input type="range" name="right" min="240" max="440" step="10" bind:value={look.right} /></label>
        <label>{tr('ความโค้งของมุม')} <output>{look.radius}px</output><input type="range" name="radius" min="0" max="20" bind:value={look.radius} /></label>
        <label>{tr('ความสูงแถวเพลง')} <output>{look.row}px</output><input type="range" name="row" min="40" max="80" step="4" bind:value={look.row} /></label>
        <label>{tr('ขนาดตัวอักษร')} <output>{look.font}%</output><input type="range" name="font" min="85" max="125" step="5" bind:value={look.font} /></label>
      </div>
      <h2>{tr('อื่นๆ')}</h2>
      <label class="switch"><input type="checkbox" name="tint" bind:checked={look.tint} /><span>{tr('แผงขวาไล่สีตามปกเพลง')}</span></label>
      <label class="switch"><input type="checkbox" name="viz" bind:checked={look.viz} /><span>{tr('แสดงคลื่นเสียงในแผง “กำลังเล่น”')}</span></label>
      <label class="switch"><input type="checkbox" name="startPanel" bind:checked={look.startPanel} /><span>{tr('เปิดแผงคิวตอนเริ่มแอป')}</span></label>
      <p><button class="ghost" onclick={() => (look = { ...DEFAULT_LOOK })}>{tr('คืนค่าเริ่มต้นทั้งหมด')}</button></p>
    {:else if pl}
      <div class="plhead">
        <span class="art big" style:background={cover(pl.name)}></span>
        <div>
          <small>{tr('เพลย์ลิสต์')}</small>
          {#if renaming}
            <form onsubmit={(e) => (e.preventDefault(), endRename())}>
              <!-- svelte-ignore a11y_autofocus -->
              <input class="rename" name="name" autocomplete="off" aria-label={tr('ชื่อเพลย์ลิสต์')} bind:value={renameVal} onblur={endRename} autofocus />
            </form>
          {:else}
            <h1>{pl.name}</h1>
          {/if}
          <p class="dim">{tr('{0} เพลง', plTracks.length)}</p>
        </div>
      </div>
      <div class="plbar">
        {#if plTracks[0]}<button class="playall" aria-label={tr('เล่นทั้งเพลย์ลิสต์')} onclick={() => play(plTracks[0], plTracks)}><Icon n="play" size={22} /></button>{/if}
        <button class="ghost" onclick={startRename}>{tr('เปลี่ยนชื่อ')}</button>
        <button class="ghost danger" onclick={askDeletePlaylist}>{tr('ลบเพลย์ลิสต์')}</button>
      </div>
      {@render rows(plTracks, pl.id)}
    {:else if auto}
      <h1>{tr(auto.name)}</h1>
      <p class="dim">{tr('เพลย์ลิสต์อัตโนมัติ คำนวณจากประวัติการฟัง · {0} เพลง', autoTracks.length)}</p>
      {#if autoTracks[0]}<button class="playall" aria-label={tr('เล่นทั้งหมด')} onclick={() => play(autoTracks[0], autoTracks)}><Icon n="play" size={22} /></button>{/if}
      {@render rows(autoTracks)}
    {/if}
  </main>

  <!-- ขวา: คิว / กำลังเล่น / เนื้อเพลง -->
  {#if panel}
    <aside class="right panel">
      {#if panel === 'now' && current}
        <div class="art huge" style:background={art(current)}></div>
        <h2>{current.title}</h2>
        <p class="dim">{tr(current.artist)} · {current.album}</p>
        {#if look.viz}<canvas bind:this={vizCanvas} class="viz" width="280" height="64" aria-hidden="true"></canvas>{/if}
      {:else if panel === 'lyrics'}
        <h2>{tr('เนื้อเพลง')}</h2>
        {#if current}<button class="ghost lybtn" onclick={openLyEdit}>{current.lrc ? tr('แก้เนื้อเพลงที่ตั้งเอง') : tr('ใส่เนื้อเพลงเอง / นำเข้า .lrc')}</button>{/if}
        {#if !current}
          <p class="dim">{tr('เล่นเพลงสักเพลงเพื่อดูเนื้อเพลง')}</p>
        {:else if ly.status === 'loading'}
          <p class="dim" role="status">{tr('กำลังค้นหาเนื้อเพลง…')}</p>
        {:else if ly.status === 'error'}
          <p class="errtxt">{ly.msg}</p>
          <button class="ghost" onclick={() => current && loadLyrics(current, true)}>{tr('ลองใหม่')}</button>
        {:else if ly.status === 'none'}
          <p class="dim">{tr('ไม่พบเนื้อเพลงของเพลงนี้ ลองแก้ชื่อเพลง/ศิลปินให้ตรง (คลิกขวา &gt; ตั้งค่าเพลง) แล้วเปิดแผงนี้ใหม่')}</p>
        {:else if ly.lines.length}
          <div class="ly">
            {#each ly.lines as l, i (i)}
              <button class="lyline" class:on={i === lyIdx} class:past={i < lyIdx} onclick={() => audio && (audio.currentTime = l.t)}>{l.text || '♪'}</button>
            {/each}
          </div>
        {:else}
          <p class="plain">{ly.plain}</p>
          <small class="dim">{tr('เนื้อเพลงนี้ไม่มีเวลากำกับ จึงเลื่อนตามเพลงไม่ได้')}</small>
        {/if}
      {:else}
        <h2>{tr('คิวเพลง')}</h2>
        {#if current}
          <small class="dim">{tr('กำลังเล่น')}</small>
          <div class="qrow now" role="presentation" oncontextmenu={(e) => menu(e, current!)}><span class="thumb" style:background={art(current)}></span><span class="two"><b>{current.title}</b><small>{tr(current.artist)}</small></span></div>
        {/if}
        <small class="dim">{tr('ถัดไป')}</small>
        {#each queue as t (t.id)}
          <button class="qrow" ondblclick={() => play(t, srcIds ? srcTracks : undefined)} oncontextmenu={(e) => menu(e, t)}>
            <span class="thumb" style:background={art(t)}></span>
            <span class="two"><b>{t.title}</b><small>{tr(t.artist)}</small></span>
          </button>
        {/each}
      {/if}
    </aside>
  {/if}

  <!-- แถบเล่นเพลง -->
  <footer class="npbar panel">
    <div class="np-left">
      {#if current}
        <span class="thumb big" style:background={art(current)}></span>
        <span class="two"><b>{current.title}</b><small>{tr(current.artist)}</small></span>
        <button class="icon heart" class:on={current.fav} aria-label={current.fav ? tr('เลิกถูกใจเพลงนี้') : tr('ถูกใจเพลงนี้')} aria-pressed={!!current.fav} onclick={() => toggleFav(current!)}><Icon n="heart" size={18} fill={!!current.fav} /></button>
      {/if}
    </div>
    <div class="np-mid">
      <div class="ctrl">
        <button class="icon" class:on={shuffle} aria-label={tr('สุ่ม')} onclick={() => (shuffle = !shuffle)}><Icon n="shuffle" /></button>
        <button class="icon" aria-label={tr('ก่อนหน้า')} onclick={prev}><Icon n="prev" /></button>
        <button class="playbtn" aria-label={playing ? tr('หยุด') : tr('เล่น')} onclick={toggle}><Icon n={playing ? 'pause' : 'play'} size={20} /></button>
        <button class="icon" aria-label={tr('ถัดไป')} onclick={next}><Icon n="next" /></button>
        <button class="icon" class:on={repeat} aria-label={tr('เล่นซ้ำ')} onclick={() => (repeat = !repeat)}><Icon n="repeat" /></button>
      </div>
      <div class="seek">
        <small>{fmt(pos)}</small>
        <input type="range" min="0" max={current?.dur ?? 0} step="1" value={pos} oninput={(e) => audio && (audio.currentTime = +e.currentTarget.value)} aria-label={tr('ตำแหน่งเพลง')} />
        <small>{fmt(current?.dur ?? 0)}</small>
      </div>
    </div>
    <div class="np-right">
      <button class="icon tab" class:on={panel === 'lyrics'} aria-label={tr('เนื้อเพลง')} title={tr('เนื้อเพลง')} onclick={() => (panel = panel === 'lyrics' ? null : 'lyrics')}><Icon n="lyrics" /><span class="lbl">{tr('เนื้อเพลง')}</span></button>
      <button class="icon tab" class:on={panel === 'queue'} aria-label={tr('คิว')} title={tr('คิว')} onclick={() => (panel = panel === 'queue' ? null : 'queue')}><Icon n="queue" /><span class="lbl">{tr('คิว')}</span></button>
      <button class="icon tab" class:on={panel === 'now'} aria-label={tr('กำลังเล่น')} title={tr('กำลังเล่น')} onclick={() => (panel = panel === 'now' ? null : 'now')}><Icon n="songs" /><span class="lbl">{tr('กำลังเล่น')}</span></button>
      <button class="icon speed" class:on={speed !== 1} aria-label={tr('ความเร็วเล่น {0} เท่า กดเพื่อเปลี่ยน', speed)} title={tr('ความเร็วเล่น')} onclick={cycleSpeed}>{speed}×</button>
      <button class="icon sleep" class:on={sleepMin > 0} aria-label={sleepMin ? tr('ตั้งเวลาปิดเสียง {0} นาที กดเพื่อเปลี่ยน', sleepMin) : tr('ตั้งเวลาปิดเสียง (ปิดอยู่)')} title={tr('ตั้งเวลาปิดเสียง')} onclick={cycleSleep}>
        <Icon n="moon" size={18} />{#if sleepMin}<small>{fmt(sleepLeft)}</small>{/if}
      </button>
      <button class="icon" aria-label={tr('เปิดมินิเพลเยอร์')} title={tr('มินิเพลเยอร์')} onclick={openMini}><Icon n="mini" size={18} /></button>
      <Icon n="volume" size={18} />
      <input class="vol" type="range" min="0" max="100" bind:value={volume} aria-label={tr('ระดับเสียง')} />
    </div>
  </footer>
</div>

{#if notice}
  <div class="toast n" role="status">
    <span class="errtxt">{notice}</span>
    <button class="ghost" onclick={() => (notice = '')}>{tr('ปิด')}</button>
  </div>
{/if}

{#if pasted}
  <div class="toast" role="status">
    <span>{tr('พบลิงก์ในคลิปบอร์ด:')} <b>{pasted.length > 48 ? pasted.slice(0, 48) + '…' : pasted}</b></span>
    <button class="primary sm" onclick={() => addDownload(pasted)}>{tr('โหลดเลย')}</button>
    <button class="ghost" onclick={() => (pasted = '')}>{tr('ปิด')}</button>
  </div>
{/if}

{#if ctx}
  <div class="ctx" role="menu" style:left="{ctx.x}px" style:top="{ctx.y}px">
    <button role="menuitem" onclick={act(() => play(ctx!.t))}>{tr('เล่น')}</button>
    <button role="menuitem" onclick={act(() => toggleFav(ctx!.t))}>{ctx.t.fav ? tr('เลิกถูกใจ') : tr('ถูกใจ')}</button>
    <button role="menuitem" onclick={() => openEdit(ctx!.t)}>{tr('ตั้งค่าเพลง…')}</button>
    {#each playlists.filter((p) => p.id !== ctx!.plId && !p.ids.includes(ctx!.t.id)) as p (p.id)}
      <button role="menuitem" onclick={act(() => addToPlaylist(p.id, ctx!.t.id))}>{tr('เพิ่มลง “{0}”', p.name)}</button>
    {/each}
    {#if ctx.plId}
      <button role="menuitem" onclick={act(() => move(ctx!.plId!, ctx!.t.id, -1))}>{tr('เลื่อนขึ้น')}</button>
      <button role="menuitem" onclick={act(() => move(ctx!.plId!, ctx!.t.id, 1))}>{tr('เลื่อนลง')}</button>
      <button role="menuitem" onclick={act(() => playlists.find((p) => p.id === ctx!.plId)?.ids.splice(playlists.find((p) => p.id === ctx!.plId)!.ids.indexOf(ctx!.t.id), 1))}>{tr('เอาออกจากเพลย์ลิสต์')}</button>
    {/if}
    <button role="menuitem" class="danger" onclick={act(() => askDeleteTrack(ctx!.t))}>{tr('ลบออกจากคลัง…')}</button>
  </div>
{/if}

<dialog bind:this={dlg} class="edit" aria-labelledby="edit-h">
  <form onsubmit={(e) => (e.preventDefault(), saveEdit())}>
    <h2 id="edit-h">{tr('ตั้งค่าเพลง')}</h2>
    <div class="editrow">
      <span class="art big" style:background={editing ? art(editing) : ''}></span>
      <div class="fields">
        <label>{tr('ชื่อเพลง')}<input name="title" autocomplete="off" bind:value={edit.title} required /></label>
        <label>{tr('ศิลปิน')}<input name="artist" autocomplete="off" bind:value={edit.artist} /></label>
        <label>{tr('อัลบั้ม')}<input name="album" autocomplete="off" bind:value={edit.album} /></label>
      </div>
    </div>
    <h3>{tr('ปกเพลง')}</h3>
    <div class="coverrow">
      <input name="cover" autocomplete="off" spellcheck="false" aria-label={tr('ลิงก์รูปหรือพาธไฟล์ปก')} placeholder={tr('วางลิงก์รูป หรือพาธไฟล์…')} bind:value={edit.src} />
      <button type="button" class="ghost" disabled={busy || !edit.src.trim()} onclick={useSource}>{tr('ใช้รูปนี้')}</button>
      <button type="button" class="ghost" disabled={busy} onclick={pickCover}>{tr('เลือกไฟล์…')}</button>
    </div>
    <button type="button" class="ghost" disabled={busy} onclick={findGenius}>{busy ? tr('กำลังทำงาน…') : tr('ค้นปกจาก Genius ด้วยชื่อและศิลปินด้านบน')}</button>
    <h3>{tr('คะแนน')}</h3>
    <div class="stars" role="group" aria-label={tr('ให้คะแนนเพลง')}>
      {#each [1, 2, 3, 4, 5] as n (n)}
        <button type="button" class="star" class:on={(editing?.rating ?? 0) >= n} aria-label={tr('{0} ดาว', n)} aria-pressed={editing?.rating === n} onclick={() => editing && (editing.rating = editing.rating === n ? 0 : n)}>★</button>
      {/each}
    </div>
    <h3>{tr('เสียง')}</h3>
    <button type="button" class="ghost" disabled={busy} onclick={cleanAudio}>{tr('ตัดความเงียบหัวท้าย + ปรับความดังให้เท่ากัน (ต้องมี ffmpeg)')}</button>
    <p class:errtxt={!editOk} class:oktxt={editOk} role="status">{editMsg}</p>
    <div class="actions">
      <button type="button" class="ghost" onclick={() => dlg?.close()}>{tr('ปิด')}</button>
      <button class="primary">{tr('บันทึก')}</button>
    </div>
  </form>
</dialog>

<dialog bind:this={lyDlg} class="edit" aria-labelledby="ly-h">
  <form onsubmit={(e) => (e.preventDefault(), saveLy())}>
    <h2 id="ly-h">{tr('เนื้อเพลงของเพลงนี้')}</h2>
    <p class="dim">{tr('วางเนื้อเพลงลงช่อง ถ้าอยากให้เลื่อนตามเวลา ใช้รูปแบบ LRC เช่น')} <code>{tr('[01:23.50] ข้อความ')}</code> {tr('ต่อบรรทัด หรือเลือกไฟล์ .lrc/.txt')}</p>
    <textarea name="lyrics" class="lytext" spellcheck="false" aria-label={tr('เนื้อเพลง')} bind:value={lyText}></textarea>
    <p class="errtxt" role="status">{lyMsg}</p>
    <div class="actions">
      <button type="button" class="ghost" onclick={pickLrc}>{tr('เลือกไฟล์…')}</button>
      {#if current?.lrc}<button type="button" class="ghost danger" onclick={() => saveLy(true)}>{tr('ลบที่ตั้งเอง (กลับไปใช้ LRCLIB)')}</button>{/if}
      <button type="button" class="ghost" onclick={() => lyDlg?.close()}>{tr('ปิด')}</button>
      <button class="primary">{tr('บันทึก')}</button>
    </div>
  </form>
</dialog>

<dialog bind:this={confirmDlg} class="edit small" aria-labelledby="cf-h">
  <h2 id="cf-h">{tr('ยืนยัน')}</h2>
  <p>{confirmS.msg}</p>
  <div class="actions">
    <button class="ghost" onclick={() => confirmDlg?.close()}>{tr('ยกเลิก')}</button>
    <button class="primary danger" onclick={() => (confirmS.ok(), confirmDlg?.close())}>{confirmS.label}</button>
  </div>
</dialog>

<dialog bind:this={palDlg} class="pal" aria-label={tr('ค้นหาด่วน')}>
  <input name="pal" role="combobox" aria-expanded="true" aria-controls="pal-list" autocomplete="off" spellcheck="false" placeholder={tr('พิมพ์ชื่อเพลง ศิลปิน เพลย์ลิสต์ หรือคำสั่ง…')} bind:value={pal.q} oninput={() => (pal.i = 0)} onkeydown={palKey} />
  <div id="pal-list" role="listbox">
    {#each palItems as it, i (it.key)}
      <button role="option" aria-selected={i === pal.i} class:on={i === pal.i} onmousemove={() => (pal.i = i)} onclick={() => (it.run(), palDlg?.close())}>
        <b>{it.label}</b><small>{it.sub}</small>
      </button>
    {:else}
      <p class="empty">{tr('ไม่พบ')}</p>
    {/each}
  </div>
</dialog>

<style>
  .app {
    display: grid;
    height: 100vh;
    padding: var(--gap);
    gap: var(--gap);
    background: var(--bg-outer);
    grid-template-columns: var(--sidebar-w) 1fr var(--right-w);
    grid-template-rows: auto 1fr var(--npbar-h);
    grid-template-areas: 'nav nav nav' 'side main right' 'bar bar bar';
  }
  .app { position: relative; isolation: isolate; }
  .bgwrap { position: fixed; inset: 0; z-index: -1; overflow: hidden; pointer-events: none; }
  .bgimg { position: absolute; inset: -40px; background-size: cover; background-position: center; }
  .panel, .app.hasbg .right { background: var(--panel-bg); }
  .bgpick { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .bgpick input { flex: 1; min-width: 200px; height: 36px; padding: 0 12px; border-radius: 6px; border: 1px solid var(--line2); background: var(--bg-raised); color: var(--text); outline: 0; }
  .app.compact { --sidebar-w: 72px; }
  .app.noright { grid-template-columns: var(--sidebar-w) 1fr; grid-template-areas: 'nav nav' 'side main' 'bar bar'; }
  .panel { border-radius: var(--radius-panel); overflow-y: auto; min-height: 0; }
  .dim { color: var(--text-dim); }
  small { color: var(--text-dim); font-size: 0.8125rem; }
  b { font-weight: 600; }
  h1 { font-size: 2rem; margin-bottom: 8px; }
  h1, h2 { text-wrap: balance; }
  h2 { font-size: 1.25rem; margin: 24px 0 12px; }

  /* nav */
  .nav { grid-area: nav; display: flex; align-items: center; gap: var(--space); padding: 4px 8px; }
  .wc { display: flex; margin: -4px -8px -4px 4px; align-self: stretch; }
  .wc button { display: grid; place-items: center; width: 46px; color: var(--text-dim); }
  .wc button:hover { background: var(--bg-hover); color: var(--text); }
  .wc button.x:hover { background: #e81123; color: #fff; }
  .logo { font-weight: 800; font-size: 1.1rem; color: var(--accent); min-width: calc(var(--sidebar-w) - 24px); text-align: left; }
  .search { display: flex; align-items: center; gap: 8px; flex: 1; max-width: 440px; margin: 0 auto; padding: 0 16px; height: 40px; border-radius: 999px; background: var(--bg-raised); color: var(--text-dim); }
  .search input { flex: 1; background: none; border: 0; outline: 0; color: var(--text); }
  .pill { display: flex; align-items: center; gap: 8px; padding: 8px 16px; border-radius: 999px; background: var(--bg-raised); }
  .pill:hover { background: var(--bg-hover); }

  /* sidebar */
  .side { grid-area: side; padding: 8px; }
  .side-top { display: flex; align-items: center; gap: 4px; padding: 8px; }
  .side-top b { flex: 1; }
  .compact .side-top { flex-direction: column; }
  .item { display: flex; align-items: center; gap: 12px; width: 100%; padding: 8px; border-radius: var(--radius-card); color: var(--text-dim); text-align: left; }
  .item:hover { background: var(--bg-raised); color: var(--text); }
  .item.active { background: var(--bg-raised); color: var(--text); }
  .compact .item span:not(.thumb) { display: none; }
  .compact .item { justify-content: center; }
  hr { border: 0; border-top: 1px solid var(--line); margin: 8px 0; }
  .two { display: flex; flex-direction: column; min-width: 0; }
  .two b, .two small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .thumb { width: 40px; height: 40px; border-radius: 4px; flex: none; display: inline-block; }
  .thumb.big { width: 56px; height: 56px; }
  .icon { display: grid; place-items: center; width: 36px; height: 36px; border-radius: 50%; color: var(--text-dim); }
  .icon:hover { color: var(--text); background: var(--bg-raised); }
  .icon.on { color: var(--accent); }

  /* main */
  .main { grid-area: main; padding: var(--space) 24px; }
  .cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(130px, 1fr)); gap: var(--space); }
  .card { display: flex; flex-direction: column; gap: 4px; padding: 12px; border-radius: var(--radius-card); background: var(--bg-raised); text-align: left; }
  .card:hover { background: var(--bg-hover); }
  .card b, .card small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .art { position: relative; aspect-ratio: 1; border-radius: 4px; margin-bottom: 8px; }
  .art.big { width: 160px; margin: 0; }
  .art.huge { width: 100%; margin-bottom: 16px; }
  .fab { position: absolute; right: 8px; bottom: 8px; display: grid; place-items: center; width: 40px; height: 40px; border-radius: 50%; background: var(--accent); color: var(--on-accent); opacity: 0; transform: translateY(6px); transition: opacity 150ms var(--ease-out), transform 150ms var(--ease-out); }
  .card:focus-visible .fab { opacity: 1; transform: none; }
  @media (hover: hover) and (pointer: fine) { .card:hover .fab { opacity: 1; transform: none; } }

  /* ตารางเพลง */
  .thead, .row { display: grid; grid-template-columns: 40px 4fr 2fr 110px 36px 60px 32px; align-items: center; gap: 12px; padding: 0 12px; }
  .thead { height: 36px; color: var(--text-dim); font-size: 0.8125rem; border-bottom: 1px solid var(--line); margin-top: 16px; }
  .row { width: 100%; height: var(--row-h); border-radius: 4px; text-align: left; }
  .row:hover, .row.sel { background: var(--bg-raised); }
  .row.now .title b { color: var(--accent); }
  .r { text-align: right; }
  .row { content-visibility: auto; contain-intrinsic-size: auto 56px; }
  .row .dim { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .title { display: flex; align-items: center; gap: 12px; min-width: 0; }
  .title > span:last-child { display: flex; flex-direction: column; min-width: 0; }
  .title b, .title small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row { overflow: hidden; }
  .num { display: grid; color: var(--text-dim); }
  .num i, .num em { grid-area: 1 / 1; font-style: normal; }
  .num em { display: none; color: var(--text); }
  .row:hover .num i { display: none; }
  .row:hover .num em { display: block; }
  .empty { padding: 24px; color: var(--text-dim); text-align: center; }

  /* playlist head */
  .plhead { display: flex; align-items: flex-end; gap: 24px; margin-bottom: 16px; }
  .playall { display: grid; place-items: center; width: 56px; height: 56px; border-radius: 50%; background: var(--accent); color: var(--on-accent); margin-top: 8px; }

  /* download */
  .dlform { display: flex; gap: 8px; margin: 16px 0 8px; }
  .dlform input { flex: 1; height: 44px; padding: 0 16px; border-radius: 8px; border: 1px solid var(--line2); background: var(--bg-raised); outline: 0; }
  .dlform input:focus { border-color: var(--accent); }
  .primary { padding: 0 24px; border-radius: 8px; background: var(--accent); color: var(--on-accent); font-weight: 700; }
  .err { color: var(--danger); margin-bottom: 8px; }
  .err:empty { display: none; }
  .errtxt { color: var(--danger); overflow-wrap: anywhere; }
  .link { color: var(--accent); text-decoration: underline; }
  .addpl { display: flex; align-items: center; gap: 8px; margin-top: 12px; color: var(--text-dim); }
  .addpl select { height: 32px; padding: 0 8px; border-radius: 6px; background-color: var(--bg-raised); color: var(--text); border: 1px solid var(--line2); }
  .primary:hover { filter: brightness(1.1); }
  .dl { display: flex; align-items: center; gap: 16px; padding: 12px; margin-top: 8px; border-radius: 8px; background: var(--bg-raised); }
  .grow { flex: 1; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .bar { height: var(--progress-h); border-radius: 2px; background: var(--line2); overflow: hidden; margin-top: 4px; }
  .bar i { display: block; height: 100%; background: var(--accent); transition: width 500ms linear; }

  /* right panel */
  .right { grid-area: right; background: linear-gradient(color-mix(in srgb, var(--tint) 38%, var(--bg-base)), var(--bg-base) 65%); padding: var(--space); display: flex; flex-direction: column; gap: 4px; }
  .right h2 { margin: 0 0 12px; overflow-wrap: anywhere; }
  .qrow { display: flex; align-items: center; gap: 12px; width: 100%; padding: 6px; border-radius: 4px; text-align: left; }
  .qrow:hover { background: var(--bg-raised); }
  .qrow.now b { color: var(--accent); }

  /* now playing bar */
  .npbar { grid-area: bar; display: grid; grid-template-columns: minmax(140px, 1fr) minmax(220px, 1.5fr) minmax(380px, 1fr); align-items: center; gap: var(--space); padding: 0 16px; overflow: hidden; }
  .np-left { display: flex; align-items: center; gap: 12px; min-width: 0; }
  .np-mid { display: flex; flex-direction: column; align-items: center; gap: 4px; }
  .ctrl { display: flex; align-items: center; gap: 8px; }
  .playbtn { display: grid; place-items: center; width: 40px; height: 40px; border-radius: 50%; background: var(--text); color: var(--bg-base); }
  .playbtn:hover { transform: scale(1.06); }
  .playbtn:active { transform: scale(0.95); }
  .seek { display: flex; align-items: center; gap: 8px; width: 100%; max-width: 560px; }
  .np-right { display: flex; align-items: center; justify-content: flex-end; gap: 4px; color: var(--text-dim); }
  input[type='range'] { accent-color: var(--accent); width: 100%; height: var(--progress-h); }
  .vol { width: 64px; }
  .tab .lbl { display: none; font-size: 0.75rem; }
  @media (min-width: 1500px) {
    .npbar { grid-template-columns: minmax(160px, 1fr) minmax(260px, 1.5fr) minmax(560px, 1fr); }
    .tab { width: auto; padding: 0 10px; gap: 6px; display: flex; align-items: center; }
    .tab .lbl { display: inline; }
    .vol { width: 96px; }
  }
  @media (max-width: 1000px) { .vol { width: 56px; } }
  .np-right { gap: 2px; min-width: 0; }
  .np-right button { flex: none; }
  .np-right > :global(svg) { flex: none; margin-left: 4px; }
  .search:focus-within { outline: 2px solid var(--accent); outline-offset: 2px; }

  /* tabular-nums: ตัวเลขเวลา/เปอร์เซ็นต์ไม่ขยับตอนเปลี่ยนค่า */
  .seek small, .num-tab, .row .r, .thead .r, .num { font-variant-numeric: tabular-nums; }

  /* ความรู้สึกตอนกด (Emil): ย่อเล็กน้อยทันทีที่กด */
  .card, .icon, .item, .pill, .primary, .playall, .playbtn { transition: transform 160ms var(--ease-out); }
  .card:active, .icon:active, .item:active, .pill:active, .primary:active, .playall:active { transform: scale(0.97); }

  /* รายการดาวน์โหลดใหม่ค่อยๆ เลื่อนลงมา แทนการโผล่พรวด */
  .dl { transition: opacity 200ms var(--ease-out), transform 200ms var(--ease-out); }
  @starting-style { .dl { opacity: 0; transform: translateY(-8px); } }

  /* เมนูคลิกขวา */
  .ctx { position: fixed; z-index: 10; min-width: 170px; padding: 4px; border-radius: 8px; background: var(--bg-hover); box-shadow: 0 8px 24px #000a; transform-origin: top left; animation: pop 120ms var(--ease-out); }
  .ctx button { display: block; width: 100%; padding: 8px 12px; border-radius: 4px; text-align: left; }
  .ctx button:hover, .ctx button:focus-visible { background: var(--line2); }
  @keyframes pop { from { opacity: 0; transform: scale(0.96); } }

  /* หน้าต่างตั้งค่าเพลง */
  .edit { margin: auto; width: min(520px, 92vw); padding: 24px; border: 0; border-radius: 12px; background: var(--bg-raised); color: var(--text); overscroll-behavior: contain; }
  .edit::backdrop { background: #000a; }
  .edit h3 { font-size: 1rem; margin: 20px 0 8px; }
  .editrow { display: flex; gap: 16px; align-items: flex-start; }
  .editrow .art.big { width: 120px; flex: none; }
  .fields { flex: 1; display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .fields label { display: flex; flex-direction: column; gap: 4px; color: var(--text-dim); font-size: 0.8125rem; }
  .edit input { height: 36px; padding: 0 12px; border-radius: 6px; border: 1px solid var(--line2); background: var(--bg-base); color: var(--text); outline: 0; min-width: 0; }
  .edit input:focus { border-color: var(--accent); }
  .coverrow { display: flex; gap: 8px; margin-bottom: 8px; }
  .coverrow input { flex: 1; }
  .ghost { padding: 8px 14px; border-radius: 6px; background: var(--bg-hover); white-space: nowrap; flex: none; }
  .ghost:hover:not(:disabled) { background: var(--line3); }
  .ghost:disabled { opacity: 0.5; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
  .actions .primary { height: 36px; padding: 0 20px; }


  .thumb.auto { display: grid; place-items: center; color: #fff; }
  .row.gone { opacity: 0.5; }
  .warn { margin: 12px 0; padding: 10px 14px; border-radius: 8px; background: #f8717122; color: var(--text); }
  .warn code { background: #0006; padding: 1px 6px; border-radius: 4px; }
  .oktxt { color: var(--accent); }
  .danger { color: var(--danger); }
  .ctx .danger:hover { background: #f8717126; }
  .primary.danger { background: var(--danger); color: #1a0505; }
  .primary.sm { height: 32px; padding: 0 14px; }
  .edit.small { width: min(420px, 92vw); }
  .edit.small p { margin: 12px 0; overflow-wrap: anywhere; }

  .tool { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 16px; margin-bottom: 8px; border-radius: 8px; background: var(--bg-raised); }
  .switch { display: flex; align-items: center; gap: 8px; cursor: pointer; flex: none; }
  .switch input { width: 18px; height: 18px; accent-color: var(--accent); }
  .tool .switch + .ghost { margin-left: -4px; }
  .tool > div { display: flex; flex-direction: column; gap: 2px; min-width: 0; }

  .plbar { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
  .plbar .playall { margin-top: 0; }
  .rename { height: 44px; font-size: 1.6rem; font-weight: 700; padding: 0 8px; border-radius: 6px; border: 1px solid var(--accent); background: var(--bg-raised); color: var(--text); outline: 0; max-width: 100%; }

  .ly { display: flex; flex-direction: column; gap: 6px; padding-bottom: 40vh; }
  .lyline { text-align: left; font-size: 1.1rem; font-weight: 700; line-height: 1.4; padding: 4px 6px; border-radius: 6px; color: var(--text-dim); opacity: 0.55; transition: color 200ms var(--ease-out), opacity 200ms var(--ease-out), font-size 200ms var(--ease-out); }
  .lyline:hover { opacity: 0.9; }
  .lyline.past { opacity: 0.4; }
  .lyline.on { color: var(--text); opacity: 1; font-size: 1.8rem; line-height: 1.25; } /* ท่อนที่กำลังร้องขยายใหญ่ ท่อนอื่นคงขนาดเดิม */
  .plain { white-space: pre-wrap; line-height: 1.6; margin-bottom: 8px; }

  .sleep { width: auto; min-width: 36px; padding: 0 6px; gap: 4px; display: flex; }
  .sleep small { font-variant-numeric: tabular-nums; color: inherit; }

  .toast { position: fixed; left: 50%; bottom: 100px; transform: translateX(-50%); z-index: 9; display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-radius: 10px; background: var(--bg-hover); box-shadow: 0 8px 24px #000a; max-width: 90vw; animation: pop 160ms var(--ease-out); }
  .toast span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .toast.n { bottom: 150px; }
  .toast.n span { white-space: normal; }

  .pal { margin: 12vh auto auto; width: min(560px, 92vw); padding: 0; border: 0; border-radius: 12px; background: var(--bg-raised); color: var(--text); overflow: hidden; }
  .pal::backdrop { background: #000a; }
  .pal input { width: 100%; height: 52px; padding: 0 20px; background: none; border: 0; border-bottom: 1px solid var(--line); color: var(--text); font-size: 1rem; outline: 0; }
  .pal [role='listbox'] { max-height: 50vh; overflow-y: auto; padding: 6px; }
  .pal [role='option'] { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; width: 100%; padding: 10px 12px; border-radius: 6px; text-align: left; }
  .pal [role='option'].on { background: var(--bg-hover); }
  .pal [role='option'] b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pal [role='option'] small { flex: none; }

  .row { cursor: default; }
  .row:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .heart { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 50%; color: var(--text-dim); opacity: 0; transition: opacity 120ms var(--ease-out), transform 160ms var(--ease-out); }
  .row:hover .heart, .row:focus-within .heart, .heart.on, .heart:focus-visible { opacity: 1; }
  .more { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 50%; color: var(--text-dim); opacity: 0; transition: opacity 120ms var(--ease-out); }
  .row:hover .more, .row:focus-within .more, .more:focus-visible { opacity: 1; }
  .more:hover { color: var(--text); background: var(--line2); }
  .heart.on { color: var(--accent); }
  .heart:hover { color: var(--text); }
  .heart:active, .icon.speed:active { transform: scale(0.92); }
  .icon.heart { opacity: 1; }
  .icon.speed { font-size: 0.75rem; font-weight: 700; width: auto; min-width: 36px; padding: 0 6px; font-variant-numeric: tabular-nums; }
  .icon.speed.on { color: var(--accent); }
  .stars { display: flex; gap: 2px; }
  .star { font-size: 1.5rem; line-height: 1; padding: 2px 4px; color: var(--text-dim); }
  .star.on { color: var(--accent); }
  .lybtn { margin: 4px 0 8px; align-self: flex-start; }
  .lytext { width: 100%; min-height: 240px; padding: 12px; border-radius: 8px; border: 1px solid var(--line2); background: var(--bg-base); color: var(--text); font: inherit; resize: vertical; }
  .errtxt { color: var(--danger); min-height: 1.2em; }
  .viz { width: 100%; height: 64px; color: var(--accent); margin-top: 8px; }
  .seg { display: flex; border: 1px solid var(--line2); border-radius: 8px; overflow: hidden; }
  .seg button { padding: 0 14px; height: 44px; color: var(--text-dim); }
  .seg button.on { background: var(--accent); color: var(--on-accent); font-weight: 700; }
  .dl.hit { display: flex; align-items: center; gap: 12px; padding: 8px 0; }
  .dl.hit .grow { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .dl.hit b, .dl.hit small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .thumb.big { width: 56px; height: 56px; border-radius: 6px; object-fit: cover; flex: none; }
  .bulk { margin: 12px 0; }
  .bulk summary { cursor: pointer; padding: 8px 0; color: var(--text-dim); }
  .bulk textarea { width: 100%; min-height: 120px; padding: 12px; border-radius: 8px; border: 1px solid var(--line2); background: var(--bg-raised); color: var(--text); font: inherit; margin-bottom: 8px; }
  .themes { display: flex; flex-wrap: wrap; gap: 12px; }
  .theme { display: flex; flex-direction: column; gap: 6px; align-items: flex-start; padding: 10px; min-width: 120px; border-radius: var(--radius-card); border: 2px solid transparent; background: var(--bg-raised); text-align: left; }
  .theme.on { border-color: var(--accent); }
  .theme small { color: var(--text-dim); }
  .theme .sw { display: flex; gap: 4px; padding: 6px; border-radius: 6px; }
  .theme .sw i { width: 22px; height: 22px; border-radius: 4px; display: block; }
  .colors { display: flex; flex-wrap: wrap; gap: 16px; }
  .colors label, .sliders label { display: flex; flex-direction: column; gap: 6px; color: var(--text-dim); }
  .colors input[type='color'] { width: 64px; height: 40px; padding: 0; border: 1px solid var(--line2); border-radius: 6px; background: none; }
  .sliders { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; margin: 16px 0; }
  .sliders output { color: var(--text); margin-left: 6px; }
  .switch { display: flex; gap: 10px; align-items: center; padding: 6px 0; }
  @media (prefers-reduced-motion: reduce) { .heart, .icon.speed { transition: none; } }
</style>