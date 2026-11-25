<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { measureLufs, normGain, detectSilence, bpmAndWave } from './loudness';
  import { Fx, corsOk, EQ_FREQS, EQ_PRESETS } from './fx';
  import { parseQuery, matches, findDuplicates, dupKey, norm } from './search';
  import { alignDetail, weakLines, fillGaps, type Word } from './align';
  import { listen, emit } from '@tauri-apps/api/event';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { getCurrentWindow, ProgressBarStatus } from '@tauri-apps/api/window';
  import { register, unregisterAll } from '@tauri-apps/plugin-global-shortcut';
  import { enable as autoOn, disable as autoOff, isEnabled as autoIs } from '@tauri-apps/plugin-autostart';
  import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';
  import { i18n, tr, type Lang } from '$lib/i18n.svelte';
  import Icon from '$lib/Icon.svelte';
  import ShareCard from '$lib/ShareCard.svelte';
  import {
    fmt, fmtDate, cover, art, hue, parseLrc, lrcIndex, splitArtistTitle, cleanLyrics, linkKey, isPlaylistLink, isSupportedLink, versionAgeDays,
    type Track, type Playlist, type Smart, type LrcLine
  } from '$lib/data';
  import { pick as spPick, query as spQuery, parseList, type SpTrack, type Hit as SpHit } from '$lib/spmatch';
  import { THEMES, LAYOUTS, DEFAULT_LOOK, normalizeLook, lookVars, isLight, paletteFromPixels, accentVars, cycleColor, cycleSeconds, cycleGradient, rgbOf, toHex, RAINBOW, type Look } from '$lib/look';

  type Done = { title: string; artist: string; album: string; dur: number; playlist: string | null; url: string; thumb: string | null; path: string; cover: string | null };

  // ---------- state ----------
  let library = $state<Track[]>([]);
  let playlists = $state<Playlist[]>([]);
  let loaded = $state(false); // กันเขียนทับไฟล์ด้วยคลังว่างก่อนอ่านเสร็จ
  const TOOL_TABS = [['gen', 'ทั่วไป'], ['snd', 'เสียง'], ['lib', 'คลังเพลง'], ['lyr', 'เนื้อเพลง'], ['net', 'เชื่อมต่อ'], ['char', 'ตัวละคร'], ['keys', 'ปุ่มลัด'], ['sys', 'ระบบ']] as const;
  // รายการปุ่มลัดทั้งหมด (ใช้ทั้งหมวดปุ่มลัดในเครื่องมือและหน้าต่างช่วยเหลือที่กด ?) — แก้ที่นี่ที่เดียว
  const KEYS: { title: string; note?: string; rows: [string, string][] }[] = [
    { title: 'ควบคุมการเล่น', rows: [['Space', 'เล่น/หยุด'], ['← →', 'ย้อน/ข้าม 5 วินาที (Shift = 15)'], ['↑ ↓', 'เพิ่ม/ลดเสียง 5%'], ['N / P', 'เพลงถัดไป / ก่อนหน้า'], ['M', 'ปิดเสียง'], ['S', 'สุ่มเล่น เปิด/ปิด'], ['R', 'เล่นซ้ำ เปิด/ปิด'], ['A', 'วนซ้ำช่วง A–B (กดครั้งแรก = จุด A, ครั้งที่สอง = จุด B, ครั้งที่สาม = ยกเลิก)']] },
    { title: 'เพลงที่กำลังเล่น', rows: [['L', 'ถูกใจ / เลิกถูกใจ'], ['0–5', 'ให้ดาว (0 = ล้างดาว)'], ['Q', 'เปิด/ปิดคิวเล่น'], ['Y', 'เปิด/ปิดเนื้อเพลง'], ['F', 'เนื้อเพลงเต็มจอ'], ['[ / ]', 'เนื้อเพลงเร็วขึ้น / ช้าลง 0.1 วินาที']] },
    { title: 'ค้นหา เลือก และลบ', rows: [['Ctrl+K', 'ค้นหาและคำสั่งทั้งหมด'], ['Ctrl+คลิก', 'เลือกเพลงเพิ่มทีละเพลง'], ['Shift+คลิก', 'เลือกเป็นช่วง'], ['Ctrl+A', 'เลือกทั้งหมดในรายการ'], ['Esc', 'ยกเลิกการเลือก / ปิดเมนู / ออกจากเต็มจอ'], ['Delete', 'ย้ายเพลงที่เลือกไปถังขยะ'], ['Ctrl+Z', 'ย้อนกลับ'], ['Ctrl+Y หรือ Ctrl+Shift+Z', 'ทำซ้ำ'], ['?', 'เปิดหน้าต่างรายการปุ่มลัด']] },
    { title: 'ตอนตั้งเวลาเนื้อเพลง (โหมด Sync)', rows: [['Space หรือ Enter', 'ประทับเวลาให้บรรทัดนี้ แล้วไปบรรทัดถัดไป'], ['Backspace', 'ถอยกลับไปบรรทัดก่อนหน้า']] },
    { title: 'ในเมนูคลิกขวาและช่องค้นหา', rows: [['↑ ↓', 'เลื่อนรายการ'], ['Enter', 'เลือกรายการ'], ['Esc', 'ปิด']] },
    { title: 'ปุ่มลัดทั้งระบบ', note: 'ใช้ได้แม้อยู่ในเกมหรือแอปอื่น (เปิด/ปิดได้ที่หมวดทั่วไป) · ปุ่มมีเดียบนคีย์บอร์ดและหูฟังก็ควบคุมเพลงได้เช่นกัน', rows: [['Ctrl+Alt+Space', 'เล่น/หยุด'], ['Ctrl+Alt+→', 'เพลงถัดไป'], ['Ctrl+Alt+←', 'เพลงก่อนหน้า'], ['Ctrl+Alt+↑', 'เพิ่มเสียง'], ['Ctrl+Alt+↓', 'ลดเสียง']] },
    { title: 'เมาส์', rows: [['คลิกเพลง', 'เล่นเพลงนั้น'], ['คลิกขวา', 'เมนูของเพลง/เพลย์ลิสต์'], ['ลากเพลง', 'ไปวางที่เพลย์ลิสต์ในแถบซ้าย หรือจัดลำดับในเพลย์ลิสต์'], ['ลากแถบด้านบน', 'ย้ายหน้าต่าง (ดับเบิลคลิก = ขยาย/คืนขนาด)'], ['ล้อเมาส์บนตัวละคร', 'ขยาย/ย่อตัวละคร (ถ้าไม่ได้ล็อก)']] }
  ];
  let toolTab = $state<(typeof TOOL_TABS)[number][0]>('gen');
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
  let badTrack = $state<Track | null>(null); // เพลงที่เล่นไม่ได้: toast มีปุ่มลบ
  let missing = $state<number[]>([]);
  let audio = $state<HTMLAudioElement>(); // เด็คที่กำลังเล่นอยู่ (มี 2 ตัวไว้ครอสเฟด)
  let decks = $state<HTMLAudioElement[]>([]);
  let activeIdx = 0;
  let fx = $state.raw<Fx | null>(null);
  let fxBusy = false, fxFail = false, vizFx = false;
  let fxMsg = $state('');
  let eqOn = $state(false);
  let eqPreset = $state('flat');
  let eqGains = $state<number[]>(EQ_PRESETS.flat.slice());
  let balance = $state(0);
  let mono = $state(false);
  let xfade = $state(0); // วินาทีครอสเฟด (0 = ปิด)
  let gapless = $state(false);
  let softFade = $state(true); // เบาเข้า/ออกนุ่มๆ ตอนกดหยุดและเล่น
  let fadeMul = $state(1);
  let abA = $state<number | null>(null);
  let abB = $state<number | null>(null);
  $effect(() => { if (!audio && decks[0]) audio = decks[0]; });
  let pendingSeek = 0;
  let look = $state<Look>({ ...DEFAULT_LOOK });
  let speed = $state(1);
  let discord = $state(true); // แสดงเพลงที่ฟังใน Discord
  let discordMsg = $state('');
  let normalize = $state(true); // ปรับระดับเสียงให้แต่ละเพลงใกล้เคียงกัน
  let uq = $state<number[]>([]); // คิวที่ผู้ใช้เพิ่มเอง (เล่นถัดไป / ต่อท้ายคิว)
  let history = $state<{ id: number; at: number }[]>([]); // ประวัติการเล่น ล่าสุดอยู่หน้า (เก็บ 200 รายการ)
  let stopAfter = $state(0); // หยุดหลังเล่นจบอีก N เพลง (0 = ปิด)
  const coverAccent = $derived(look.accentMode === 'cover'); // สีเน้นตามปกเพลงที่เล่นอยู่
  let accentOv = $state<string | null>(null);
  let flash = $state('');
  let npMore = $state(false); // เมนู … ขวาล่าง (fixed เพราะแถบเล่นตัดส่วนที่ล้น)
  let npMoreAt = $state({ right: 16, bottom: 100 });
  let helpOpen = $state(false);
  let nowFull = $state(false); // หน้า “กำลังเล่น” เต็มหน้าต่าง
  let mainEl = $state<HTMLElement>();
  let scrollY = $state(0), viewH = $state(800), rowsTop = $state(0);
  let drag = $state<{ ids: number[]; x: number; y: number; label: string; over: number | null; overPl: number | null; plId?: number } | null>(null);
  let seekHover = $state<{ x: number; t: number } | null>(null);
  let selIds = $state<number[]>([]); // เพลงที่เลือกไว้หลายเพลง (Ctrl/Shift+คลิก)
  let selAnchor = -1;
  type TrashItem = { t: Track; at: number; pls: [number, number][] };
  let trash = $state<TrashItem[]>([]);
  let smart = $state<Smart[]>([]);
  let dupIgnore = $state<string[]>([]);
  let sortKey = $state(''); // '' = ลำดับเดิม
  let sortDir = $state<1 | -1>(1);
  let cols = $state({ album: true, added: true, plays: false, rating: false, bpm: false, dur: true });
  type Alarm = { on: boolean; time: string; pl: string; ramp: boolean };
  let alarm = $state<Alarm>({ on: false, time: '07:00', pl: '', ramp: true });
  type Follow = { url: string; name: string; seen: string[]; at: number };
  let follows = $state<Follow[]>([]);
  let lfm = $state({ key: '', secret: '', sk: '', user: '', on: false });
  // Spotify: อ่านรายชื่อเพลงแล้วหาบน YouTube (Client ID ของผู้ใช้เอง ไม่ใช่ความลับ เก็บใน prefs ได้)
  let spCid = $state('');
  let spUser = $state<string | null>(null);
  let spBusy = $state(false), spMsg = $state('');
  let spPls = $state<{ id: string; name: string; count: number; image: string | null; mine: boolean }[] | null>(null);
  type SpItem = { t: SpTrack; st: 'wait' | 'search' | 'ok' | 'have' | 'pick' | 'none' | 'skip'; alts?: SpHit[] };
  let pasteList = $state(''), pasteName = $state('');
  // ตัวละครบนหน้าจอ: เก็บได้หลายตัว แสดงทีละตัว (ตำแหน่ง/ขนาดจำแยกต่อตัว)
  type Pet = { id: string; name: string; idle: string; hover: string; press: string; size: number; locked: boolean; x?: number; y?: number };
  let pets = $state<Pet[]>([]);
  let petId = $state<string | null>(null);
  let petOn = $state(false), petBounce = $state(false), petLife = $state(true), petLy = $state(false), petMsg = $state('');
  let spImp = $state<{ name: string; plId: number; items: SpItem[]; stop: boolean; done: boolean } | null>(null);
  let autoUpd = $state(false); // ติดตั้งอัปเดตแบบเงียบให้เองเมื่อไม่ได้เล่นเพลง
  let autoYt = $state(true); // อัปเดต yt-dlp ให้เองเมื่อเก่าเกิน 14 วัน
  let remoteOn = $state(false);
  let remoteInfo = $state<{ urls: string[]; pin: string } | null>(null);
  let watchDir = $state(''); // โฟลเดอร์เฝ้าดู: ไฟล์เพลงใหม่ในนี้จะถูกนำเข้าเอง
  let watchSeen = $state<string[]>([]);
  let dragOver = $state(false);
  let closeToTray = $state(true); // กดปิดหน้าต่าง = ซ่อนไปถาดระบบ เพลงเล่นต่อ
  let hotkeys = $state(true); // ปุ่มลัดทั้งระบบ (ใช้ได้แม้อยู่ในแอปอื่น)
  let notifyOn = $state(true); // แจ้งเตือน Windows ตอนเปลี่ยนเพลง (เฉพาะตอนแอปไม่ได้อยู่หน้าสุด)
  let taskProg = $state(true); // แถบความคืบหน้าบนไอคอนในแถบงาน
  let autoStart = $state(false);
  let trayHinted = false;
  let barLy = $state(true); // แสดงบรรทัดเนื้อเพลงที่กำลังร้องในแถบเล่นด้านล่าง
  let ytCookies = $state(''); // คุกกี้ให้ yt-dlp: '' | firefox | edge | chrome | brave | file:พาธ
  let discordLy = $state(false); // แสดงบรรทัดเนื้อเพลงใต้ชื่อเพลงในสถานะ Discord
  let dLine = $state(''); // บรรทัดที่ส่งไป Discord ล่าสุด (จำกัดความถี่)
  let seeks = $state(0); // นับครั้งที่กรอเพลง เพื่อให้ Discord ปรับแถบเวลา

  // ---------- derived ----------
  // สีพื้นไอคอนเพลย์ลิสต์อัตโนมัติ: สุ่มจากชื่อ (สีเดิมทุกครั้งที่เปิดแอป) ตัวอักษรขาวยังอ่านออก
  const plTile = (p: Playlist) => (p.image ? `center / cover no-repeat url("${convertFileSrc(p.image)}")` : tileColor(p.name));
  const tileColor = (name: string) => {
    const h = (hue(name) * 7 + 23) % 360;
    // ไล่สีจากมุมบนซ้าย (สว่าง) ไปมุมล่างขวา (เข้มและเหลื่อมสีนิดหน่อย) ไอคอนขาวยังอ่านออกทั้งสองฝั่ง
    return `linear-gradient(135deg, hsl(${h} 72% 52%), hsl(${(h + 38) % 360} 68% 30%))`;
  };
  const artistOf = (t: Track) => tr(t.artist);
  const pq = $derived(parseQuery(query));
  const shown = $derived(library.filter((t) => matches(t, pq, artistOf)));
  const smartCur = $derived(view.startsWith('smart:') ? smart.find((x) => x.id === +view.slice(6)) : undefined);
  const smartTracks = $derived(smartCur ? library.filter((t) => matches(t, parseQuery(smartCur.q), artistOf)) : []);
  const artistName = $derived(view.startsWith('artist:') ? view.slice(7) : '');
  const albumName = $derived(view.startsWith('album:') ? view.slice(6) : '');
  const artistTracks = $derived(artistName ? library.filter((t) => t.artist === artistName) : []);
  const albumTracks = $derived(albumName ? library.filter((t) => t.album === albumName) : []);
  const artists = $derived.by(() => {
    const m = new Map<string, Track[]>();
    for (const t of library) m.set(t.artist, [...(m.get(t.artist) ?? []), t]);
    return [...m].map(([name, ts]) => ({ name, ts })).sort((a, b) => b.ts.length - a.ts.length || a.name.localeCompare(b.name));
  });
  const albums = $derived.by(() => {
    const m = new Map<string, Track[]>();
    for (const t of library) if (t.album?.trim()) m.set(t.album, [...(m.get(t.album) ?? []), t]);
    return [...m].map(([name, ts]) => ({ name, ts })).sort((a, b) => b.ts.length - a.ts.length);
  });
  const dups = $derived(view === 'dups' ? findDuplicates(library).filter((g) => !dupIgnore.includes(dupKey(g[0]))) : []);
  const SORTERS: Record<string, (a: Track, b: Track) => number> = {
    title: (a, b) => a.title.localeCompare(b.title), artist: (a, b) => artistOf(a).localeCompare(artistOf(b)),
    album: (a, b) => (a.album ?? '').localeCompare(b.album ?? ''), added: (a, b) => a.added.localeCompare(b.added),
    plays: (a, b) => a.plays - b.plays, bpm: (a, b) => (a.bpm ?? 0) - (b.bpm ?? 0), rating: (a, b) => (a.rating ?? 0) - (b.rating ?? 0), dur: (a, b) => a.dur - b.dur
  };
  const sorted = (list: Track[]) => (sortKey && SORTERS[sortKey] ? [...list].sort((a, b) => SORTERS[sortKey](a, b) * sortDir) : list);
  function sortBy(k: string) {
    if (sortKey !== k) { sortKey = k; sortDir = k === 'added' || k === 'plays' || k === 'rating' ? -1 : 1; }
    else if (sortDir === (k === 'added' || k === 'plays' || k === 'rating' ? -1 : 1)) sortDir = sortDir === 1 ? -1 : 1;
    else sortKey = '';
  }
  const colsTpl = $derived(['40px', 'minmax(0,4fr)', cols.album && 'minmax(0,2fr)', cols.added && '110px', cols.plays && '56px', cols.rating && '84px', cols.bpm && '56px', '36px', cols.dur && '60px', '32px'].filter(Boolean).join(' '));
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
    { id: 'new', name: 'เพิ่งโหลด 7 วัน' },
    { id: 'skip', name: 'ข้ามบ่อย' },
    { id: 'mix', name: 'มิกซ์ประจำวัน' }
  ];
  const auto = $derived(view.startsWith('auto:') ? AUTO.find((a) => a.id === view.slice(5)) : undefined);
  const autoTracks = $derived.by(() => {
    if (!auto) return [];
    if (auto.id === 'fav') return library.filter((t) => t.fav);
    if (auto.id === 'top') return library.filter((t) => (t.rating ?? 0) >= 4).sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0));
    if (auto.id === 'freq') return library.filter((t) => t.plays > 0).sort((a, b) => b.plays - a.plays).slice(0, 50);
    if (auto.id === 'old') return [...library].sort((a, b) => a.last.localeCompare(b.last)).slice(0, 50);
    if (auto.id === 'mix') return dailyMix();
    if (auto.id === 'skip') return library.filter((t) => (t.skips ?? 0) >= 3 && (t.skips ?? 0) >= t.plays * 0.6).sort((a, b) => (b.skips ?? 0) - (a.skips ?? 0));
    const since = new Date(Date.now() - 7 * 864e5).toISOString().slice(0, 10);
    return library.filter((t) => t.added >= since).sort((a, b) => b.added.localeCompare(a.added));
  });

  // มิกซ์ประจำวัน: สุ่มแบบคงที่ทั้งวัน เน้นเพลงที่ชอบ + ศิลปิน/แท็กที่ฟังบ่อย + เพลงที่ไม่ได้ฟังนาน
  function dailyMix(): Track[] {
    const day = new Date().toISOString().slice(0, 10);
    let seed = [...day].reduce((a, c) => (a * 31 + c.charCodeAt(0)) >>> 0, 7);
    const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
    const pool = library.filter((t) => !missing.includes(t.id) && (t.skips ?? 0) < 3);
    const top = [...pool].sort((a, b) => b.plays - a.plays).slice(0, 15);
    const favArtists = new Set(top.map((t) => t.artist)), favTags = new Set(top.flatMap((t) => t.tags ?? []));
    const score = (t: Track) =>
      (t.fav ? 3 : 0) + (t.rating ?? 0) + (favArtists.has(t.artist) ? 2 : 0) + ((t.tags ?? []).some((x) => favTags.has(x)) ? 1 : 0) +
      Math.min(3, (Date.now() - new Date(t.last || t.added).getTime()) / 864e5 / 10) + rnd() * 4;
    const out: Track[] = [];
    const perArtist = new Map<string, number>();
    for (const t of pool.map((t) => [t, score(t)] as const).sort((a, b) => b[1] - a[1]).map((x) => x[0])) {
      const n = perArtist.get(t.artist) ?? 0;
      if (n >= 3) continue; // ไม่ให้ศิลปินเดียวเยอะเกินไป
      perArtist.set(t.artist, n + 1);
      out.push(t);
      if (out.length >= 30) break;
    }
    return out;
  }
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
        smart = Array.isArray(d.smart) ? d.smart : [];
        dupIgnore = Array.isArray(d.dupIgnore) ? d.dupIgnore : [];
        // ถังขยะ: เกิน 30 วันลบไฟล์จริง
        const tr0: TrashItem[] = Array.isArray(d.trash) ? d.trash : [];
        const expired = tr0.filter((x) => Date.now() - x.at > 30 * 864e5);
        if (expired.length) invoke('delete_files', { paths: expired.flatMap((x) => [x.t.path, ...(x.t.cover ? [x.t.cover] : [])]) }).catch(() => {});
        trash = tr0.filter((x) => !expired.includes(x));
        const p = d.prefs ?? {};
        volume = p.volume ?? volume;
        shuffle = !!p.shuffle;
        repeat = !!p.repeat;
        discord = p.discord ?? true;
        discordLy = !!p.discordLy;
        normalize = p.normalize ?? true;
        barLy = p.barLy ?? true;
        if (p.alarm && typeof p.alarm === 'object') alarm = { ...alarm, ...p.alarm };
        follows = Array.isArray(p.follows) ? p.follows : [];
        if (p.lfm && typeof p.lfm === 'object') lfm = { ...lfm, ...p.lfm };
        if (typeof p.spCid === 'string') spCid = p.spCid;
        if (Array.isArray(p.pets)) pets = p.pets.filter((x: Pet) => x && typeof x.id === 'string').map((x: Pet) => ({ ...x, size: Math.min(800, Math.max(48, +x.size || 200)), locked: !!x.locked }));
        petId = typeof p.petId === 'string' && pets.some((x) => x.id === p.petId) ? p.petId : (pets[0]?.id ?? null);
        petOn = !!p.petOn;
        petBounce = !!p.petBounce; petLife = p.petLife ?? true; petLy = !!p.petLy;
        autoUpd = !!p.autoUpd; autoYt = p.autoYt ?? true;
        watchDir = typeof p.watchDir === 'string' ? p.watchDir : ''; watchSeen = Array.isArray(p.watchSeen) ? p.watchSeen : [];
        closeToTray = p.closeToTray ?? true; hotkeys = p.hotkeys ?? true; notifyOn = p.notifyOn ?? true; taskProg = p.taskProg ?? true; trayHinted = !!p.trayHinted;
        if (p.cols && typeof p.cols === 'object') cols = { ...cols, ...p.cols };
        sortKey = typeof p.sortKey === 'string' ? p.sortKey : ''; sortDir = p.sortDir === -1 ? -1 : 1;
        eqOn = !!p.eqOn; eqPreset = typeof p.eqPreset === 'string' ? p.eqPreset : 'flat';
        if (Array.isArray(p.eqGains) && p.eqGains.length === 10) eqGains = (p.eqGains as unknown[]).map((x) => Number(x) || 0);
        balance = +p.balance || 0; mono = !!p.mono; xfade = +p.xfade || 0; gapless = !!p.gapless; softFade = p.softFade ?? true;
        history = Array.isArray(p.history) ? p.history.slice(0, 200) : [];
        ytCookies = typeof p.ytCookies === 'string' ? p.ytCookies : '';
        speed = p.speed ?? 1;
        if (p.lang === 'th' || p.lang === 'en') i18n.lang = p.lang;
        look = normalizeLook(p.look);
        if (p.coverAccent && look.accentMode === 'solid') look.accentMode = 'cover'; // ย้ายจากสวิตช์เดิมในหน้าเครื่องมือ
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
    await listen<{ id: number; pct: number; title: string | null; item: { idx: number; n: number; list: string | null; title: string } | null }>('dl-progress', (e) => {
      const d = downloads.find((x) => x.id === e.payload.id);
      if (!d || d.state === 'done' || d.state === 'failed') return; // เหตุการณ์ที่มาช้าหลังงานจบ ไม่ย้อนสถานะ
      const it = e.payload.item;
      if (it) {
        d.cur = it.title;
        if (it.n > 0) { d.idx = it.idx; d.n = it.n; }
        if (it.list && d.playlist) d.title = it.list;
      } else if (e.payload.title && !d.playlist) d.title = e.payload.title;
      // เพลย์ลิสต์: แถบรวมทั้งลิสต์ (เพลงที่เสร็จแล้ว + ความคืบหน้าเพลงนี้) ไม่วิ่งวนกลับ 0 ทุกเพลง
      if (e.payload.pct > 0) d.pct = d.n ? Math.min(99, (((d.idx ?? 1) - 1 + e.payload.pct / 100) / d.n) * 100) : e.payload.pct;
      d.state = e.payload.pct >= 100 ? 'converting' : 'downloading';
    });
    await listen<[number, string[] | number, string?]>('dl-skip', (e) => {
      const d = downloads.find((x) => x.id === e.payload[0]);
      if (!d) return;
      const errs = Array.isArray(e.payload[1]) ? e.payload[1] : Array(e.payload[1]).fill(e.payload[2] ?? '');
      d.skipped = errs.length;
      // รวมสาเหตุที่เหมือนกันเป็นกลุ่ม: "Video unavailable × 4"
      const g = new Map<string, number>();
      for (const x of errs) g.set(x, (g.get(x) ?? 0) + 1);
      d.why = [...g].sort((a, b) => b[1] - a[1]).map(([m, n]) => ({ m: tr(m) || tr('ไม่ทราบสาเหตุ'), n }));
    });
    await listen<[number, Done]>('dl-done', (e) => onTrackDone(...e.payload));
    await listen<{ cmd: string; v?: number }>('np-cmd', (e) => {
      const { cmd, v } = e.payload;
      if (cmd === 'toggle') toggle();
      else if (cmd === 'next') next();
      else if (cmd === 'prev') prev();
      else if (cmd === 'seek' && audio && v !== undefined) audio.currentTime = v;
      else if (cmd === 'vol' && v != null) volume = Math.max(0, Math.min(100, Math.round(v)));
      else if (cmd === 'play' && v != null) { const t = library.find((x) => x.id === v); if (t) play(t); }
    });
    await listen('np-ready', () => pushNp());
  });
  // shortcut: เขียนทั้งไฟล์ทุกครั้งที่แก้ (หน่วง 400ms), พอคลังเกินหลักพันเพลงค่อยย้ายไป SQLite
  $effect(() => {
    if (!loaded) return;
    const json = JSON.stringify({
      tracks: $state.snapshot(library),
      playlists: $state.snapshot(playlists),
      smart: $state.snapshot(smart),
      trash: $state.snapshot(trash),
      dupIgnore: $state.snapshot(dupIgnore),
      prefs: { volume, shuffle, repeat, discord, discordLy, ytCookies, normalize, barLy, alarm: $state.snapshot(alarm), follows: $state.snapshot(follows), lfm: $state.snapshot(lfm), spCid, pets: $state.snapshot(pets), petId, petOn, petBounce, petLife, petLy, autoUpd, autoYt, watchDir, watchSeen: $state.snapshot(watchSeen), closeToTray, hotkeys, notifyOn, taskProg, trayHinted, cols: $state.snapshot(cols), sortKey, sortDir, eqOn, eqPreset, eqGains: $state.snapshot(eqGains), balance, mono, xfade, gapless, softFade, history: $state.snapshot(history), speed, lang: i18n.lang, look: $state.snapshot(look), resumeId: current?.id ?? null, resumePos }
    });
    const timer = setTimeout(() => invoke('write_library', { json }).catch((e) => (notice = tr('บันทึกไม่ได้: {0}', tr(String(e))))), 400);
    return () => clearTimeout(timer);
  });

  // ---------- player ----------
  let skipFired = false;
  function beginTrack(t: Track, list?: Track[], keepSrc = false) {
    if (!keepSrc) srcIds = list ? list.map((x) => x.id) : null;
    current = t;
    pos = resumePos = 0;
    skipFired = false;
    abA = abB = null;
    // เพลงยาว (มิกซ์/พอดแคสต์) เล่นต่อจากที่ค้างไว้ ไม่งั้นข้ามช่วงเงียบต้นเพลงที่ตั้งไว้
    pendingSeek = t.dur > 900 && t.resume && t.resume > 10 && t.resume < t.dur - 15 ? t.resume : t.skipStart || 0;
    if (history[0]?.id !== t.id || Date.now() - history[0].at > 5000) {
      history.unshift({ id: t.id, at: Date.now() });
      if (history.length > 200) history.length = 200;
    }
    notice = ''; badTrack = null;
    t.plays += 1;
    t.last = new Date().toISOString().slice(0, 10);
  }
  function play(t: Track, list?: Track[], keepSrc = false) {
    if (!audio) return;
    if (missing.includes(t.id)) {
      notice = tr('ไฟล์ของ “{0}” หายไป ลองลบออกจากคลังแล้วโหลดใหม่', t.title);
      return;
    }
    plan = null;
    beginTrack(t, list, keepSrc);
    if (fx) { fx.solo(activeIdx); decks[1 - activeIdx]?.pause(); xfading = false; }
    clearInterval(fadeTimer); // ไม่งั้นเฟดออกของการกดหยุดค้างอยู่ แล้วไปสั่งหยุดเพลงใหม่
    fadeMul = 1;
    delete audio.dataset.src; // เด็คนี้ถูกใช้โหลดเพลงอื่นแล้ว ป้ายพรีโหลดเดิมใช้ไม่ได้
    audio.src = convertFileSrc(t.path);
    audio.play().catch(() => {});
  }
  let fadeTimer = 0;
  function rampMul(to: number, ms: number, done?: () => void) {
    clearInterval(fadeTimer);
    const from = fadeMul, t0 = performance.now();
    fadeTimer = window.setInterval(() => {
      const k = Math.min(1, (performance.now() - t0) / ms);
      fadeMul = from + (to - from) * k;
      if (k >= 1) { clearInterval(fadeTimer); done?.(); }
    }, 20);
  }
  function toggle() {
    if (!audio || !current) return;
    if (audio.paused) {
      if (softFade && !document.hidden) fadeMul = 0;
      audio.play().catch(() => {});
      if (softFade && !document.hidden) rampMul(1, 250);
    } else if (softFade && !document.hidden) {
      rampMul(0, 180, () => { audio?.pause(); fadeMul = 1; });
    } else audio.pause();
  }
  // สุ่มแบบฉลาด: เพลงดาวเยอะมีโอกาสมากกว่า, เลี่ยงศิลปินเดิมติดกัน และเพลงที่เพิ่งเล่น 5 เพลงล่าสุด
  function pickShuffle(): Track | undefined {
    const cur = current!;
    const recent = new Set(history.slice(0, 5).map((h) => h.id));
    let pool = srcTracks.filter((t) => t.id !== cur.id && !missing.includes(t.id));
    const fresh = pool.filter((t) => !recent.has(t.id));
    if (fresh.length) pool = fresh;
    const other = pool.filter((t) => t.artist !== cur.artist);
    if (other.length) pool = other;
    const w = pool.map((t) => 1 + (t.rating ?? 0));
    let r = Math.random() * w.reduce((a, b) => a + b, 0);
    for (let i = 0; i < pool.length; i++) if ((r -= w[i]) < 0) return pool[i];
    return pool[0];
  }
  let plan: { for: number; t: Track; q: boolean } | null = null;
  function peekNext(): Track | undefined {
    if (!current) return;
    while (uq.length && !library.some((x) => x.id === uq[0])) uq.shift();
    if (uq.length) {
      const t = library.find((x) => x.id === uq[0])!;
      plan = { for: current.id, t, q: true };
      return t;
    }
    if (plan && plan.for === current.id && !plan.q) return plan.t;
    const t = shuffle ? pickShuffle() : queue.find((x) => !missing.includes(x.id));
    plan = t ? { for: current.id, t, q: false } : null;
    return t;
  }
  function next(user: unknown = true) {
    if (!current) return;
    // กดข้ามเองก่อนเล่นถึง 30% = นับว่าข้าม (ไว้หาเพลงที่ไม่ค่อยอยากฟัง)
    if (user !== false && audio && current.dur > 30 && audio.currentTime < current.dur * 0.3) current.skips = (current.skips ?? 0) + 1;
    const t = peekNext();
    if (!t) { if (user === false) playing = false; return; } // กดข้ามเองแต่ไม่มีเพลงถัดไป: เพลงเดิมเล่นต่อ ไม่ให้ปุ่มเพี้ยน
    const fromQ = !!plan?.q;
    plan = null;
    if (fromQ) uq.shift();
    play(t, fromQ ? undefined : srcIds ? srcTracks : undefined, fromQ);
  }
  // ---------- ย้อนกลับ/ทำซ้ำ: เก็บภาพคลังก่อนแก้ไข (ไม่บันทึกลงดิสก์) ----------
  let undoS: string[] = [], redoS: string[] = [];
  let undoN = $state(0);
  const snap = () => JSON.stringify({ library, playlists, trash, smart });
  function checkpoint() {
    undoS.push(snap());
    if (undoS.length > 30) undoS.shift();
    redoS = [];
    undoN++;
  }
  function restoreSnap(j: string) {
    const d = JSON.parse(j);
    library = d.library; playlists = d.playlists; trash = d.trash; smart = d.smart;
    if (current) current = library.find((t) => t.id === current!.id) ?? current;
    selIds = selIds.filter((id) => library.some((t) => t.id === id));
    undoN++;
  }
  function undo() {
    const j = undoS.pop();
    if (!j) return say(tr('ไม่มีอะไรให้ย้อนกลับ'));
    redoS.push(snap());
    restoreSnap(j);
    say(tr('ย้อนกลับแล้ว'));
  }
  function redo() {
    const j = redoS.pop();
    if (!j) return say(tr('ไม่มีอะไรให้ทำซ้ำ'));
    undoS.push(snap());
    restoreSnap(j);
    say(tr('ทำซ้ำแล้ว'));
  }
  // ---------- ถังขยะ ----------
  function toTrash(ts: Track[]) {
    if (!ts.length) return;
    checkpoint();
    const ids = new Set(ts.map((t) => t.id));
    if (current && ids.has(current.id)) {
      audio?.pause();
      audio?.removeAttribute('src');
      current = null;
    }
    for (const t of ts) {
      const pls: [number, number][] = playlists.flatMap((p) => (p.ids.includes(t.id) ? [[p.id, p.ids.indexOf(t.id)] as [number, number]] : []));
      trash.unshift({ t: $state.snapshot(t) as Track, at: Date.now(), pls });
    }
    library = library.filter((t) => !ids.has(t.id));
    for (const p of playlists) p.ids = p.ids.filter((i) => !ids.has(i));
    uq = uq.filter((i) => !ids.has(i));
    selIds = selIds.filter((i) => !ids.has(i));
    selected = null;
    say(tr('ย้าย {0} เพลงไปถังขยะแล้ว', ts.length), { label: tr('ย้อนกลับ'), fn: undo });
  }
  function restoreTrash(items: TrashItem[]) {
    checkpoint();
    for (const it of items) {
      if (!library.some((t) => t.id === it.t.id)) library.unshift(it.t);
      for (const [pid, idx] of it.pls) {
        const p = playlists.find((x) => x.id === pid);
        if (p && !p.ids.includes(it.t.id)) p.ids.splice(Math.min(idx, p.ids.length), 0, it.t.id);
      }
    }
    const set = new Set(items);
    trash = trash.filter((x) => !set.has(x));
  }
  function purgeTrash(items: TrashItem[]) {
    ask(tr('ลบ {0} เพลงถาวร? ไฟล์จะถูกลบจากเครื่อง ย้อนกลับไม่ได้', items.length), tr('ลบถาวร'), () => {
      const set = new Set(items);
      invoke('delete_files', { paths: items.flatMap((x) => [x.t.path, ...(x.t.cover ? [x.t.cover] : [])]) }).catch(() => {});
      trash = trash.filter((x) => !set.has(x));
      undoS = []; redoS = []; // ไฟล์หายแล้ว ย้อนกลับไม่ได้
    });
  }
  // ---------- เลือกหลายเพลง ----------
  function rowClick(e: MouseEvent, t: Track, i: number, list: Track[]) {
    if (e.ctrlKey || e.metaKey) {
      selIds = selIds.includes(t.id) ? selIds.filter((x) => x !== t.id) : [...selIds, t.id];
      selAnchor = i;
      return;
    }
    if (e.shiftKey && selAnchor >= 0) {
      const [a, b] = [Math.min(selAnchor, i), Math.max(selAnchor, i)];
      selIds = [...new Set([...selIds, ...list.slice(a, b + 1).map((x) => x.id)])];
      return;
    }
    selIds = [t.id];
    selAnchor = i;
    selected = t.id;
    play(t, list);
  }
  const selTracks = $derived(selIds.flatMap((id) => library.find((t) => t.id === id) ?? []));
  const vis = { list: [] as Track[] }; // รายการที่แสดงอยู่ (ไว้ให้ Ctrl+A)
  function bulkRate(n: number) { checkpoint(); for (const t of selTracks) t.rating = n; say(tr('ให้ {0} ดาวกับ {1} เพลง', n, selTracks.length)); }
  function bulkFav(on: boolean) { checkpoint(); for (const t of selTracks) t.fav = on; }
  function bulkToPlaylist(v: string) {
    if (!v) return;
    const pid = v === 'new' ? newPlaylist() : +v;
    const p = playlists.find((x) => x.id === pid);
    if (!p) return;
    checkpoint();
    let added = 0;
    for (const t of selTracks) if (!p.ids.includes(t.id)) { p.ids.push(t.id); added++; }
    const dup = selTracks.length - added;
    say(dup ? tr('เพิ่ม {0} เพลงลง “{1}” (ข้าม {2} เพลงที่มีอยู่แล้ว)', added, p.name, dup) : tr('เพิ่ม {0} เพลงลง “{1}”', added, p.name));
  }
  let bulkDlg = $state<HTMLDialogElement>();
  let bed = $state({ artist: '', album: '', addTags: '', delTags: '' });
  function openBulk() { bed = { artist: '', album: '', addTags: '', delTags: '' }; openDlg(bulkDlg); }
  const splitTags = (s: string) => s.split(/[,，]/).map((x) => x.trim()).filter(Boolean);
  function saveBulk() {
    checkpoint();
    const add = splitTags(bed.addTags), del = splitTags(bed.delTags).map((x) => x.toLowerCase());
    for (const t of selTracks) {
      if (bed.artist.trim()) t.artist = bed.artist.trim();
      if (bed.album.trim()) t.album = bed.album.trim();
      if (add.length || del.length) t.tags = [...new Set([...(t.tags ?? []), ...add])].filter((x) => !del.includes(x.toLowerCase()));
      dropLy(t.id);
    }
    bulkDlg?.close();
    say(tr('แก้ไข {0} เพลงแล้ว', selTracks.length));
  }
  let coverBusy = $state('');
  async function bulkCovers() {
    const todo = selTracks.filter((t) => !t.cover);
    let ok = 0;
    for (const [i, t] of todo.entries()) {
      coverBusy = `${i + 1}/${todo.length}`;
      try {
        const r = await invoke<{ path: string; url: string | null }>('genius_cover', { key: String(t.id), query: `${artistOf(t)} ${t.title}`.trim(), old: null });
        t.cover = r.path; t.coverUrl = r.url; ok++;
      } catch { /* ไม่เจอปก ข้าม */ }
    }
    coverBusy = '';
    say(tr('หาปกได้ {0} จาก {1} เพลง', ok, todo.length));
  }
  // ---------- เพลงซ้ำ ----------
  function resolveDup(g: Track[], keep: Track) {
    checkpoint();
    const others = g.filter((t) => t !== keep);
    keep.plays += others.reduce((a, t) => a + t.plays, 0);
    keep.fav = keep.fav || others.some((t) => t.fav);
    keep.rating = Math.max(keep.rating ?? 0, ...others.map((t) => t.rating ?? 0));
    keep.tags = [...new Set([keep.tags ?? [], ...others.map((t) => t.tags ?? [])].flat())];
    for (const p of playlists) p.ids = [...new Set(p.ids.map((id) => (others.some((o) => o.id === id) ? keep.id : id)))];
    undoS.pop(); // รวมเป็นขั้นเดียวกับ toTrash ด้านล่าง
    toTrash(others);
  }
  // ---------- พื้นที่เก็บข้อมูล / ซ่อมไฟล์ ----------
  type Store = { audio: number; covers: number; other: number; library: number; orphans: string[]; orphan_bytes: number };
  let store = $state<Store | null>(null);
  let storeMsg = $state('');
  const keepPaths = () => [...library, ...trash.map((x) => x.t)].flatMap((t) => [t.path, ...(t.cover ? [t.cover] : [])]).concat(look.bg ? [look.bg] : []).concat(playlists.flatMap((p) => (p.image ? [p.image] : [])));
  async function loadStore() {
    storeMsg = '';
    try { store = await invoke<Store>('storage_info', { keep: keepPaths() }); } catch (e) { storeMsg = tr(String(e)); }
  }
  const mb = (b: number) => new Intl.NumberFormat(i18n.lang === 'th' ? 'th-TH' : 'en-US', { maximumFractionDigits: 1 }).format(b / 1048576) + ' MB';
  function cleanOrphans() {
    if (!store?.orphans.length) return;
    ask(tr('ลบไฟล์ {0} ไฟล์ที่ไม่มีเพลงไหนใช้แล้ว ({1})?', store.orphans.length, mb(store.orphan_bytes)), tr('ลบไฟล์'), async () => {
      await invoke('delete_files', { paths: store!.orphans }).catch(() => {});
      loadStore();
    });
  }
  let relinkMsg = $state('');
  async function repairMissing() {
    const dir = await open({ directory: true, title: tr('เลือกโฟลเดอร์ที่มีไฟล์เพลงที่หาย') });
    if (typeof dir !== 'string') return;
    const gone = library.filter((t) => missing.includes(t.id));
    const base = (p: string) => p.split(/[\\/]/).pop() ?? p;
    relinkMsg = tr('กำลังค้นหา…');
    try {
      const found = await invoke<[string, string][]>('relink', { dir, names: gone.flatMap((t) => [base(t.path), ...(t.cover ? [base(t.cover)] : [])]) });
      const m = new Map(found.map(([n, p]) => [n.toLowerCase(), p]));
      let n = 0;
      for (const t of gone) {
        const np = m.get(base(t.path).toLowerCase());
        if (np) { t.path = np; n++; }
        const nc = t.cover && m.get(base(t.cover).toLowerCase());
        if (nc) t.cover = nc;
      }
      const left = await invoke<string[]>('missing_files', { paths: library.map((t) => t.path) });
      missing = library.filter((t) => left.includes(t.path)).map((t) => t.id);
      relinkMsg = tr('เชื่อมกลับได้ {0} จาก {1} เพลง', n, gone.length);
    } catch (e) {
      relinkMsg = tr(String(e));
    }
  }
  function saveSmart() {
    const id = Date.now();
    smart.push({ id, name: query.trim(), q: query.trim() });
    view = `smart:${id}`;
  }
  // แจ้งเตือนสั้นๆ; ใส่ act ได้ เช่น ปุ่มย้อนกลับ (ค้างนานขึ้นให้กดทัน และค้างไว้ตอนเอาเมาส์ชี้)
  let flashAct = $state<{ label: string; fn: () => void } | null>(null);
  let flashT = 0, flashHold = false;
  function say(m: string, act?: { label: string; fn: () => void }) {
    flash = m;
    flashAct = act ?? null;
    clearTimeout(flashT);
    const hide = () => { if (flashHold) flashT = setTimeout(hide, 800) as unknown as number; else { flash = ''; flashAct = null; } };
    flashT = setTimeout(hide, act ? 6000 : 2200) as unknown as number;
  }
  const flashDo = () => { const a = flashAct; flash = ''; flashAct = null; a?.fn(); };
  function queueNext(t: Track) { uq.unshift(t.id); say(tr('จะเล่น “{0}” เป็นเพลงถัดไป', t.title), { label: tr('ย้อนกลับ'), fn: () => { const i = uq.indexOf(t.id); if (i >= 0) uq.splice(i, 1); } }); }
  function queueAdd(t: Track) { uq.push(t.id); say(tr('เพิ่ม “{0}” เข้าคิวแล้ว', t.title), { label: tr('ย้อนกลับ'), fn: () => { const i = uq.lastIndexOf(t.id); if (i >= 0) uq.splice(i, 1); } }); }
  function prev() {
    if (!current || !audio) return;
    if (audio.currentTime > 3) audio.currentTime = 0;
    else {
      const h = shuffle ? library.find((x) => x.id === history[1]?.id && !missing.includes(x.id)) : undefined; // สุ่มอยู่: ย้อนไปเพลงที่เพิ่งฟัง
      if (h) { history.shift(); play(h, srcIds ? srcTracks : undefined); }
      else play(queue[queue.length - 1] ?? current, srcIds ? srcTracks : undefined);
    }
  }
  function ended() {
    if (current) current.resume = 0;
    if (stopAfter > 0 && --stopAfter === 0) { audio?.pause(); return; } // ตั้งให้หยุดหลังจบเพลง (ข้ามท้ายเพลงมาแล้วเสียงยังเดินอยู่ ต้องหยุดเอง)
    abA = abB = null;
    if (repeat && current) return play(current, srcIds ? srcTracks : undefined);
    next(false);
  }
  function onTime() {
    if (!audio) return;
    pos = audio.currentTime;
    if (abB !== null && pos >= abB) { audio.currentTime = abA ?? 0; return; }
    const d = audio.duration;
    if (current?.skipEnd && isFinite(d) && !skipFired && pos >= d - current.skipEnd - 0.05 && !((xfade > 0 || gapless) && fx)) {
      skipFired = true; // ข้ามความเงียบท้ายเพลง
      ended();
      return;
    }
    if (Math.floor(pos / 5) !== Math.floor(resumePos / 5)) {
      resumePos = pos;
      if (current && current.dur > 900) current.resume = Math.floor(pos);
    }
  }
  function onMeta() {
    if (audio && pendingSeek) audio.currentTime = pendingSeek;
    pendingSeek = 0;
  }
  $effect(() => {
    if (!audio) return;
    // เร็ว/ช้าแบบเทป: คีย์เปลี่ยนตามความเร็ว (แบบ nightcore / slowed) แทนการยืดเวลาที่เสียงแตกเป็นคลื่น
    for (const d of new Set([audio, ...decks])) { d.preservesPitch = false; d.defaultPlaybackRate = speed; d.playbackRate = speed; }
  });
  const SPEEDS = [0.8, 0.9, 1, 1.1, 1.25, 1.5];
  const cycleSpeed = () => (speed = SPEEDS[(SPEEDS.indexOf(speed) + 1) % SPEEDS.length]);

  // ---------- ตัวแสดงคลื่นเสียง: ฟังเสียงผ่าน captureStream (ไม่ต่อเข้าทางออกเสียง จึงไม่มีทางทำให้เพลงเงียบ) ----------
  let vizCanvas = $state<HTMLCanvasElement>();
  let actx: AudioContext | undefined;
  let analyser: AnalyserNode | undefined;
  let vsrc: MediaStreamAudioSourceNode | undefined;
  let vizSrc = '';
  function attachViz() {
    if (vizFx || !audio || (!look.viz && !(petOn && petBounce)) || vizSrc === audio.currentSrc) return;
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
      const col = getComputedStyle(c).color;
      g.fillStyle = g.strokeStyle = col;
      if (look.vizStyle === 'wave') {
        g.lineWidth = 2.5; g.lineJoin = 'round';
        g.beginPath();
        for (let i = 0; i < 48; i++) {
          const v = data[i] / 255, x = (i / 47) * c.width, y = c.height / 2 - (i % 2 ? -1 : 1) * v * (c.height / 2 - 3);
          if (i) g.lineTo(x, y); else g.moveTo(x, y);
        }
        g.stroke();
      } else if (look.vizStyle === 'circle') {
        const cx = c.width / 2, cy = c.height / 2, r0 = c.height * 0.22, n = 40;
        g.lineWidth = 3; g.lineCap = 'round';
        for (let i = 0; i < n; i++) {
          const a = (i / n) * Math.PI * 2 - Math.PI / 2, v = data[Math.floor(i * 1.2)] / 255, r1 = r0 + 3 + v * (c.height / 2 - r0 - 3);
          g.beginPath(); g.moveTo(cx + Math.cos(a) * r0, cy + Math.sin(a) * r0); g.lineTo(cx + Math.cos(a) * r1, cy + Math.sin(a) * r1); g.stroke();
        }
      } else {
        const n = 28, w = c.width / n;
        for (let i = 0; i < n; i++) {
          const h = Math.max(3, (data[Math.floor(i * 1.5)] / 255) * c.height);
          g.fillRect(i * w + 1, c.height - h, w - 3, h);
        }
      }
      raf = requestAnimationFrame(draw);
    };
    draw();
    return () => cancelAnimationFrame(raf);
  });
  // ถ้า WebView ไม่ยอมให้อ่านไฟล์แบบ CORS: ปิดคลื่นเสียง แล้วโหลดเพลงเดิมต่อโดยไม่ใช้ CORS
  let corsTried: { viz: boolean } | null = null;
  function audioError() {
    if (!fx && audio?.crossOrigin && current) {
      const at = audio.currentTime;
      corsTried = { viz: look.viz };
      audio.removeAttribute('crossorigin');
      look.viz = false;
      pendingSeek = at;
      audio.src = convertFileSrc(current.path);
      audio.play().catch(() => {});
      return;
    }
    playing = false;
    // ลองแบบไม่ใช้ CORS แล้วยังพังอีก = ปัญหาอยู่ที่ไฟล์ ไม่ใช่ WebView: คืนค่าที่เราเพิ่งปิดไป
    if (corsTried && audio) { audio.crossOrigin = 'anonymous'; look.viz = corsTried.viz; corsTried = null; }
    const bad = current;
    if (bad) invoke<string[]>('missing_files', { paths: [bad.path] }).then((m) => { if (m.length && !missing.includes(bad.id)) missing.push(bad.id); }).catch(() => {});
    badTrack = current;
    notice = tr('เล่นไฟล์นี้ไม่ได้ ไฟล์อาจถูกย้ายหรือลบ');
  }

  let fading = false;
  // ระดับเสียงจริง = ที่ผู้ใช้ตั้ง × ตัวคูณปรับเสียงรายเพลง (ลดเพลงที่ดังเกิน)
  // เพลงที่ยังไม่เคยวัดความดัง: เริ่มเบาไว้ก่อน (-6dB) แล้วค่อยไล่ไปค่าจริงเมื่อวัดเสร็จ ไม่งั้นเพลงเสียงดังจะดังเต็มที่ช่วงแรกแล้วหล่นฮวบ
  let lufsFailed = $state<number[]>([]);
  const gainTarget = $derived(
    (normalize ? (current && current.lufs == null && !lufsFailed.includes(current.id) ? 0.5 : normGain(current?.lufs)) : 1) * 10 ** ((current?.gainDb ?? 0) / 20)
  );
  let gain = $state(1);
  let gainFor = -1, gainRaf = 0;
  $effect(() => {
    const to = gainTarget, id = current?.id ?? -1;
    cancelAnimationFrame(gainRaf);
    if (id !== gainFor || Math.abs(to - gain) < 0.01) { gainFor = id; gain = to; return; } // เปลี่ยนเพลง = ตั้งทันที
    const from = gain, t0 = performance.now();
    const step = (now: number) => {
      const k = Math.min(1, (now - t0) / 900);
      gain = from + (to - from) * k;
      if (k < 1) gainRaf = requestAnimationFrame(step);
    };
    gainRaf = requestAnimationFrame(step);
  });
  const vol = () => Math.min(1, (volume / 100) * gain);
  // fx เปิดอยู่: คุมเสียงที่ master gain (เด็คคงที่ที่ 1) ไม่งั้นคุมที่ <audio> ตรงๆ
  function applyVol(mul = 1) {
    const v = vol() * mul;
    if (fx) { fx.setMaster(v); for (const d of decks) d.volume = 1; }
    else if (audio) audio.volume = v;
  }
  $effect(() => {
    void [volume, gain, fx, fadeMul];
    if (audio && !fading) applyVol(fadeMul);
  });
  // วัดความดังของเพลงที่เพิ่งเล่นครั้งแรก (ครั้งเดียวต่อเพลง เก็บผลไว้ในคลัง)
  const lufsTried = new Set<number>();
  async function analyze(t: Track): Promise<void> {
    if ((t.lufs != null && t.bpm !== undefined && t.wave !== undefined) || lufsTried.has(t.id)) return;
    lufsTried.add(t.id);
    try {
      const bytes = await (await fetch(convertFileSrc(t.path))).arrayBuffer();
      if (t.bpm === undefined || t.wave === undefined) {
        const r = await bpmAndWave(bytes.slice(0)).catch(() => ({ bpm: null, wave: '' }));
        t.bpm = r.bpm; t.wave = r.wave;
      }
      const v = t.lufs != null ? t.lufs : await measureLufs(bytes);
      if (v != null) t.lufs = Math.round(v * 10) / 10;
    } catch {
      /* ถอดรหัสไม่ได้: ข้าม เล่นด้วยระดับเสียงปกติ */
    }
    if (t.lufs == null) lufsFailed.push(t.id); // วัดไม่ได้ ก็เลิกเบาไว้ก่อน
  }
  $effect(() => {
    const t = current;
    if (t && ((normalize && t.lufs == null) || t.bpm === undefined || t.wave === undefined)) analyze(t);
  });
  // ---------- อัปเดตแอป (เช็กรีลีสล่าสุดบน GitHub) ----------
  type Upd = { version: string; url: string; page: string };
  let upd = $state<Upd | null>(null);
  let updBusy = $state(false);
  let updMsg = $state('');
  let updDismissed = $state(false);
  async function checkUpdate(manual: boolean) {
    if (updBusy) return;
    updBusy = true;
    if (manual) updMsg = tr('กำลังตรวจ…');
    try {
      const u = await invoke<Upd | null>('check_update');
      upd = u;
      if (manual) updMsg = u ? tr('มีเวอร์ชันใหม่ {0}', u.version) : tr('เป็นเวอร์ชันล่าสุดแล้ว');
      if (u && autoUpd && !manual) { const go = () => (!playing ? installUpdate(true) : setTimeout(go, 60000)); go(); }
    } catch (e) {
      if (manual) updMsg = tr(String(e));
    }
    updBusy = false;
  }
  async function installUpdate(silent = false) {
    if (!upd || updBusy) return;
    updBusy = true;
    updMsg = tr('กำลังโหลดตัวติดตั้ง… แอปจะปิดแล้วเปิดตัวติดตั้งให้เอง');
    try {
      await invoke('install_update', { url: upd.url, silent });
    } catch (e) {
      updMsg = tr(String(e));
      notice = updMsg;
      updBusy = false;
    }
  }
  let updChecked = false;
  $effect(() => {
    if (!loaded || updChecked) return;
    updChecked = true;
    const h = setTimeout(() => checkUpdate(false), 4000); // เช็กเงียบๆ หลังเปิดแอป ถ้าเน็ตไม่มีก็ข้ามไป
    return () => clearTimeout(h);
  });
  // สีเน้นตามปกเพลงที่กำลังเล่น (ไม่เขียนทับธีมที่ตั้งไว้ ปิดแล้วกลับเป็นสีเดิม)
  $effect(() => {
    const c = coverAccent ? current?.cover : null;
    if (!c) { accentOv = null; return; }
    let dead = false;
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      try {
        const cv = document.createElement('canvas');
        cv.width = cv.height = 48;
        const x = cv.getContext('2d')!;
        x.drawImage(img, 0, 0, 48, 48);
        if (!dead) accentOv = paletteFromPixels(x.getImageData(0, 0, 48, 48).data).accent;
      } catch { if (!dead) accentOv = null; }
    };
    img.onerror = () => !dead && (accentOv = null);
    img.src = convertFileSrc(c);
    return () => (dead = true);
  });
  let tidyMsg = $state('');
  function tidyTitles() {
    checkpoint();
    let n = 0;
    for (const t of library) {
      const sp = splitArtistTitle(t.title);
      if (sp) { t.title = sp.title; t.artist = sp.artist; n++; }
    }
    tidyMsg = n ? tr('แก้แล้ว {0} เพลง', n) : tr('ไม่มีเพลงที่ต้องแก้');
  }
  let analyzing = $state<{ done: number; total: number } | null>(null);
  async function analyzeAll() {
    const todo = library.filter((t) => (t.lufs == null || t.bpm === undefined) && !missing.includes(t.id));
    analyzing = { done: 0, total: todo.length };
    for (const t of todo) {
      lufsTried.delete(t.id);
      await analyze(t);
      analyzing = { done: analyzing.done + 1, total: todo.length };
    }
    analyzing = null;
  }

  // Space = เล่น/หยุด ยกเว้นตอนพิมพ์ข้อความหรืออยู่ในหน้าต่างย่อย; กันปุ่มที่โฟกัสอยู่ถูกกดซ้ำ
  const typing = (e: KeyboardEvent) => {
    const el = e.target as HTMLInputElement;
    return !!el.closest?.('dialog') || el.tagName === 'INPUT' || el.tagName === 'SELECT';
  };
  function onKey(e: KeyboardEvent) {
    if (sync) return syncKey(e); // โหมดตั้งเวลาเนื้อเพลง: Space/Enter/Backspace ใช้กับตัวตั้งเวลา
    if (e.key === 'Escape') { ctx = null; plCtx = null; helpOpen = false; if (karaoke) closeKaraoke(); if (nowFull) setNowFull(false); }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      return openPalette();
    }
    const onRange = (e.target as HTMLInputElement)?.type === 'range';
    if (e.code === 'Space') {
      if (typing(e) && !onRange) return;
      e.preventDefault();
      return toggle();
    }
    if ((e.ctrlKey || e.metaKey) && !typing(e) && !document.querySelector('dialog[open]')) {
      const k2 = e.key.toLowerCase();
      if (k2 === 'z' && !e.shiftKey) { e.preventDefault(); return undo(); }
      if ((k2 === 'z' && e.shiftKey) || k2 === 'y') { e.preventDefault(); return redo(); }
      if (k2 === 'a' && vis.list.length) { e.preventDefault(); selIds = vis.list.map((t) => t.id); return; }
    }
    if (e.key === 'Escape' && selIds.length > 1) { selIds = []; return; }
    if (e.key === 'Delete' && selTracks.length && !typing(e) && !document.querySelector('dialog[open]')) return toTrash(selTracks);
    if (e.ctrlKey || e.metaKey || e.altKey || typing(e) || document.querySelector('dialog[open]')) return;
    const k = e.key;
    const seekBy = (d: number) => audio && current && (audio.currentTime = Math.max(0, Math.min(current.dur || audio.duration || 0, audio.currentTime + d)));
    if ((k === 'ArrowLeft' || k === 'ArrowRight') && !onRange) { e.preventDefault(); seekBy((k === 'ArrowLeft' ? -1 : 1) * (e.shiftKey ? 15 : 5)); }
    else if ((k === 'ArrowUp' || k === 'ArrowDown') && !onRange && !(e.target as HTMLElement)?.closest?.('select')) { e.preventDefault(); volume = Math.max(0, Math.min(100, volume + (k === 'ArrowUp' ? 5 : -5))); }
    else if (k === 'n' || k === 'N') next();
    else if (k === 'p' || k === 'P') prev();
    else if (k === 'm' || k === 'M') toggleMute();
    else if (k === 's' || k === 'S') { shuffle = !shuffle; say(shuffle ? tr('สุ่ม: เปิด') : tr('สุ่ม: ปิด')); }
    else if (k === 'r' || k === 'R') { repeat = !repeat; say(repeat ? tr('เล่นซ้ำ: เปิด') : tr('เล่นซ้ำ: ปิด')); }
    else if ((k === 'l' || k === 'L') && current) { toggleFav(current); heartPop(document.querySelector('.npbar .heart'), !!current.fav); }
    else if (/^[0-5]$/.test(k) && current) { current.rating = +k; say(+k ? '★'.repeat(+k) : tr('ล้างดาวแล้ว')); }
    else if (k === 'a' || k === 'A') toggleAB();
    else if (k === '[') nudgeLy(-0.1);
    else if (k === ']') nudgeLy(0.1);
    else if (k === 'q' || k === 'Q') panel = panel === 'queue' ? null : 'queue';
    else if (k === 'y' || k === 'Y') panel = panel === 'lyrics' ? null : 'lyrics';
    else if (k === '?') helpOpen = !helpOpen;
    else if ((k === 'f' || k === 'F') && current) (karaoke ? closeKaraoke() : openKaraoke());
  }
  // ชื่อยาว: ตอนชี้เมาส์ให้เลื่อนวิ่งให้เห็นครบ
  function marquee(node: HTMLElement) {
    const on = () => {
      const o = node.scrollWidth - node.clientWidth;
      if (o > 4) { node.style.setProperty('--mq', `-${o}px`); node.style.setProperty('--mqt', `${Math.max(2, o / 40)}s`); node.classList.add('mq'); }
    };
    const off = () => node.classList.remove('mq');
    node.addEventListener('mouseenter', on); node.addEventListener('mouseleave', off);
    return { destroy() { node.removeEventListener('mouseenter', on); node.removeEventListener('mouseleave', off); } };
  }
  // ---------- เอฟเฟกต์เสียง: สร้างกราฟ Web Audio เมื่อเปิดใช้ EQ/โมโน/บาลานซ์/ครอสเฟดเท่านั้น ----------
  async function ensureFx() {
    if (fx || fxBusy || fxFail || decks.length < 2) return;
    fxBusy = true;
    try {
      const src = audio?.currentSrc || (current ? convertFileSrc(current.path) : '');
      if (src && !(await corsOk(src))) {
        fxFail = true;
        fxMsg = tr('เครื่องนี้ไม่อนุญาตให้ประมวลผลเสียง เอฟเฟกต์จึงใช้ไม่ได้');
        return;
      }
      const f = new Fx([decks[0], decks[1]]);
      f.solo(activeIdx);
      f.resume();
      vsrc?.disconnect();
      actx = f.ctx;
      analyser = Object.assign(f.ctx.createAnalyser(), { fftSize: 128, smoothingTimeConstant: 0.8 });
      f.master.connect(analyser);
      vizFx = true;
      fx = f;
      fxMsg = '';
    } catch (e) {
      fxFail = true;
      fxMsg = tr(String(e));
    } finally {
      fxBusy = false;
    }
  }
  const fxWanted = $derived(eqOn || mono || balance !== 0 || xfade > 0 || gapless);
  $effect(() => { if (fxWanted && !fx && !fxFail) ensureFx(); });
  $effect(() => {
    if (!fx) return;
    fx.setEq(eqOn ? eqGains : null);
    fx.setBalance(balance);
    fx.setMono(mono);
  });
  function pickPreset(name: string) {
    eqPreset = name;
    if (EQ_PRESETS[name]) eqGains = EQ_PRESETS[name].slice();
  }
  // ครอสเฟด/เล่นต่อเนื่อง: โหลดเพลงถัดไปรอไว้ที่เด็คว่าง แล้วเริ่มก่อนจบตามเวลาที่ตั้ง
  let xfading = false;
  function preloadNext(n: Track) {
    const el = decks[1 - activeIdx];
    const url = convertFileSrc(n.path);
    if (el && el.dataset.src !== url) { el.src = url; el.dataset.src = url; el.preload = 'auto'; }
  }
  function xfadeTo(n: Track, fromQ: boolean, sec: number) {
    if (!fx || !audio) return;
    const from = activeIdx, to = 1 - from, el = decks[to], old = decks[from];
    xfading = true;
    preloadNext(n);
    try { el.currentTime = n.skipStart || 0; } catch { /* ยังไม่พร้อม เริ่มที่ต้นเพลง */ }
    el.playbackRate = speed;
    fx.crossfade(from, to, sec);
    el.play().catch(() => {});
    activeIdx = to;
    audio = el;
    if (fromQ) uq.shift();
    if (stopAfter > 1) stopAfter--;
    plan = null;
    beginTrack(n, fromQ ? undefined : srcIds ? srcTracks : undefined, fromQ);
    pendingSeek = 0;
    setTimeout(() => { old.pause(); xfading = false; }, sec * 1000 + 150);
  }
  function tickX() {
    if (!fx || !audio || !current || xfading || audio.paused || repeat || stopAfter === 1) return;
    const sec = xfade > 0 ? xfade : gapless ? 0.08 : 0;
    const d = audio.duration;
    if (!sec || !isFinite(d) || d <= 0) return;
    const rem = d - (current.skipEnd ?? 0) - audio.currentTime;
    if (rem > 15) return;
    const n = peekNext();
    if (!n) return;
    preloadNext(n);
    if (rem <= sec + 0.02) xfadeTo(n, !!plan?.q, Math.max(0.05, Math.min(sec, rem)));
  }
  $effect(() => {
    if (!fx) return;
    const h = setInterval(tickX, 60);
    return () => clearInterval(h);
  });
  function toggleAB() {
    if (!audio) return;
    if (abA === null) abA = audio.currentTime;
    else if (abB === null) { abB = audio.currentTime; if (abB <= abA + 0.5) abB = null; }
    else abA = abB = null;
  }
  let trimBusy = $state(false);
  let trimMsg = $state('');
  async function detectTrim() {
    if (!editing || trimBusy) return;
    trimBusy = true;
    trimMsg = tr('กำลังตรวจ…');
    try {
      const bytes = await (await fetch(convertFileSrc(editing.path))).arrayBuffer();
      const r = await detectSilence(bytes);
      if (!r) throw new Error('long');
      editing.skipStart = r.start > 0.3 ? Math.round(r.start * 10) / 10 : 0;
      editing.skipEnd = r.end > 0.3 ? Math.round(r.end * 10) / 10 : 0;
      trimMsg = tr('ตรวจแล้ว: ต้นเพลงเงียบ {0} วินาที ท้ายเพลงเงียบ {1} วินาที', editing.skipStart, editing.skipEnd);
    } catch {
      trimMsg = tr('ตรวจความเงียบของไฟล์นี้ไม่ได้');
    }
    trimBusy = false;
  }
  // ---------- ระบบ Windows: ถาดระบบ / ปุ่มลัดทั้งระบบ / เปิดพร้อมเครื่อง / แจ้งเตือน / แถบงาน ----------
  onMount(() => {
    const w = getCurrentWindow();
    const un = w.onCloseRequested((e) => {
      if (!closeToTray) return;
      e.preventDefault();
      w.hide();
      if (!trayHinted) {
        trayHinted = true;
        if (notifyOn) notify('MySound', tr('ยังเล่นเพลงอยู่ในถาดระบบ คลิกขวาที่ไอคอนเพื่อออกจากแอป'));
      }
    });
    autoIs().then((v) => (autoStart = v)).catch(() => {});
    return () => void un.then((f) => f());
  });
  $effect(() => {
    if (!loaded) return;
    if (!hotkeys) { unregisterAll().catch(() => {}); return; }
    const keys: [string, () => void][] = [
      ['CommandOrControl+Alt+Space', toggle], ['CommandOrControl+Alt+Right', () => next()], ['CommandOrControl+Alt+Left', prev],
      ['CommandOrControl+Alt+Up', () => (volume = Math.min(100, volume + 5))], ['CommandOrControl+Alt+Down', () => (volume = Math.max(0, volume - 5))]
    ];
    (async () => {
      await unregisterAll().catch(() => {});
      for (const [k, f] of keys) await register(k, (e) => e.state === 'Pressed' && f()).catch(() => {});
    })();
    return () => void unregisterAll().catch(() => {});
  });
  async function setAutoStart(on: boolean) {
    try { if (on) await autoOn(); else await autoOff(); autoStart = await autoIs(); } catch (e) { notice = tr(String(e)); }
  }
  async function notify(title: string, body: string) {
    try {
      let ok = await isPermissionGranted();
      if (!ok) ok = (await requestPermission()) === 'granted';
      if (ok) sendNotification({ title, body });
    } catch { /* ไม่มีสิทธิ์แจ้งเตือน: ข้าม */ }
  }
  let lastNotified = 0;
  $effect(() => {
    const t = current;
    if (!t || !notifyOn || !playing || lastNotified === t.id) return;
    lastNotified = t.id;
    if (!document.hasFocus()) notify(t.title, tr(t.artist));
  });
  $effect(() => {
    const t = current;
    invoke('tray_tooltip', { text: t ? `${t.title} — ${tr(t.artist)}` : 'MySound' }).catch(() => {});
  });
  let lastProg = -1;
  $effect(() => {
    const w = getCurrentWindow();
    if (!taskProg || !current || !current.dur) { if (lastProg !== -2) { lastProg = -2; w.setProgressBar({ status: ProgressBarStatus.None }).catch(() => {}); } return; }
    const pct = Math.round((pos / current.dur) * 100);
    const key = pct * 2 + (playing ? 1 : 0);
    if (key === lastProg) return;
    lastProg = key;
    w.setProgressBar({ status: playing ? ProgressBarStatus.Normal : ProgressBarStatus.Paused, progress: pct }).catch(() => {});
  });
  // ---------- นำเข้าไฟล์จากเครื่อง ----------
  type Imp = { src: string; title: string; artist: string; album: string; dur: number; path: string; cover: string | null; lrc: string | null };
  async function importPaths(paths: string[]): Promise<Track[]> {
    const audio0 = paths.filter((p) => /\.(m4a|mp3|webm|opus|wav|flac|ogg|aac|mp4)$/i.test(p));
    if (!audio0.length) return [];
    say(tr('กำลังนำเข้า {0} ไฟล์…', audio0.length));
    let rs: Imp[] = [];
    try { rs = await invoke<Imp[]>('import_files', { paths: audio0 }); } catch (e) { notice = tr(String(e)); return []; }
    const today = new Date().toISOString().slice(0, 10);
    const added: Track[] = [];
    for (const r of rs) {
      const have = library.find((t) => t.path === r.path);
      if (have) { added.push(have); continue; }
      const sp = !r.artist ? splitArtistTitle(r.title) : null;
      const t: Track = { id: Date.now() + seq++, src: 'local', added: today, last: today, plays: 0, title: sp?.title ?? r.title, artist: sp?.artist ?? (r.artist || 'ไม่ทราบศิลปิน'), album: r.album, dur: Math.round(r.dur), path: r.path, cover: r.cover, ...(r.lrc ? { lrc: r.lrc } : {}) };
      library.unshift(t);
      added.push(library[0]);
    }
    say(tr('นำเข้าแล้ว {0} เพลง', added.length));
    return added;
  }
  async function openFiles(paths: string[]) {
    const lists = paths.filter((p) => /\.(m3u8?|mysound|json|csv)$/i.test(p));
    for (const l of lists) await importListFile(l);
    const ts = await importPaths(paths);
    if (ts[0]) play(ts[0], ts);
  }
  async function pickImport() {
    const f = await open({ multiple: true, filters: [{ name: tr('ไฟล์เพลง'), extensions: ['mp3', 'm4a', 'flac', 'wav', 'ogg', 'opus', 'aac', 'webm'] }] });
    if (Array.isArray(f)) importPaths(f);
    else if (typeof f === 'string') importPaths([f]);
  }
  onMount(() => {
    const un = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === 'enter' || p.type === 'over') dragOver = true;
      else if (p.type === 'leave') dragOver = false;
      else if (p.type === 'drop') { dragOver = false; openFiles(p.paths); }
    });
    const un2 = listen<string[]>('open-files', (e) => openFiles(e.payload));
    return () => { void un.then((f) => f()); void un2.then((f) => f()); };
  });
  let ytChecked = false;
  $effect(() => {
    if (!loaded || ytChecked || !autoYt || ytAge === null) return;
    ytChecked = true;
    if (ytAge > 14) invoke<string>('ytdlp_update').then(() => refreshTools()).catch(() => {});
  });
  let launched = false;
  $effect(() => {
    if (!loaded || launched) return;
    launched = true;
    invoke<string[]>('launch_files').then((f) => { if (f?.length) openFiles(f); }).catch(() => {});
  });
  // ---------- โฟลเดอร์เฝ้าดู ----------
  async function scanWatch() {
    if (!watchDir) return;
    const all = await invoke<string[]>('scan_folder', { dir: watchDir }).catch(() => [] as string[]);
    const seen = new Set(watchSeen);
    const fresh = all.filter((p) => !seen.has(p));
    if (!fresh.length) return;
    watchSeen = [...watchSeen, ...fresh].slice(-20000);
    await importPaths(fresh);
  }
  $effect(() => {
    if (!loaded || !watchDir) return;
    scanWatch();
    const h = setInterval(scanWatch, 60000);
    return () => clearInterval(h);
  });
  async function pickWatch() {
    const d = await open({ directory: true, title: tr('เลือกโฟลเดอร์ที่จะให้แอปเฝ้าดู') });
    if (typeof d !== 'string') return;
    // ไฟล์ที่มีอยู่แล้วตอนเลือก: ถามก่อนว่าจะนำเข้าเลยไหม
    const all = await invoke<string[]>('scan_folder', { dir: d }).catch(() => [] as string[]);
    watchSeen = all; // ไฟล์เดิมนับว่าเห็นแล้ว (ถ้าผู้ใช้ตอบนำเข้า ค่อยล้างแล้วสแกนใหม่)
    watchDir = d;
    if (all.length) ask(tr('ในโฟลเดอร์นี้มีไฟล์เพลง {0} ไฟล์ นำเข้าเลยไหม? (ถ้าไม่ จะนำเข้าเฉพาะไฟล์ใหม่ที่เพิ่มทีหลัง)', all.length), tr('นำเข้าเลย'), () => { watchSeen = []; scanWatch(); });
  }
  // ---------- สำรอง/กู้คืน / เพลย์ลิสต์ไฟล์ ----------
  const today = () => new Date().toISOString().slice(0, 10);
  let fileMsg = $state('');
  async function backup() {
    const f = await save({ defaultPath: `MySound-backup-${today()}.json`, filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (!f) return;
    const data = { app: 'MySound', kind: 'backup', v: 1, at: new Date().toISOString(), tracks: $state.snapshot(library), playlists: $state.snapshot(playlists), smart: $state.snapshot(smart) };
    try { await invoke('save_text', { path: f, text: JSON.stringify(data, null, 1) }); fileMsg = tr('สำรองแล้ว: {0}', f); } catch (e) { fileMsg = tr(String(e)); }
  }
  async function restore() {
    const f = await open({ filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (typeof f !== 'string') return;
    try {
      const d = JSON.parse(await invoke<string>('load_text', { path: f }));
      if (d?.app !== 'MySound' || !Array.isArray(d.tracks)) throw new Error(tr('ไฟล์นี้ไม่ใช่ไฟล์สำรองของ MySound'));
      ask(tr('กู้คืนจากไฟล์สำรอง? คลังปัจจุบัน ({0} เพลง) จะถูกแทนที่ด้วย {1} เพลงในไฟล์ (Ctrl+Z ย้อนกลับได้)', library.length, d.tracks.length), tr('กู้คืน'), async () => {
        checkpoint();
        library = d.tracks; playlists = Array.isArray(d.playlists) ? d.playlists : []; smart = Array.isArray(d.smart) ? d.smart : [];
        const m = await invoke<string[]>('missing_files', { paths: library.map((t) => t.path) }).catch(() => [] as string[]);
        missing = library.filter((t) => m.includes(t.path)).map((t) => t.id);
        fileMsg = tr('กู้คืนแล้ว {0} เพลง', library.length) + (missing.length ? ' · ' + tr('{0} เพลงหาไฟล์ไม่เจอ ใช้ “ซ่อมไฟล์ที่หาย” ได้', missing.length) : '');
      });
    } catch (e) { fileMsg = tr(String(e instanceof Error ? e.message : e)); }
  }
  async function exportM3u(p: Playlist) {
    const f = await save({ defaultPath: `${p.name}.m3u8`, filters: [{ name: 'M3U', extensions: ['m3u8', 'm3u'] }] });
    if (!f) return;
    const text = '#EXTM3U\n' + byIds(p.ids).map((t) => `#EXTINF:${Math.round(t.dur)},${tr(t.artist)} - ${t.title}\n${t.path}`).join('\n') + '\n';
    try { await invoke('save_text', { path: f, text }); say(tr('ส่งออกแล้ว')); } catch (e) { notice = tr(String(e)); }
  }
  async function sharePlaylist(p: Playlist) {
    const f = await save({ defaultPath: `${p.name}.mysound`, filters: [{ name: 'MySound playlist', extensions: ['mysound'] }] });
    if (!f) return;
    const data = { app: 'MySound', kind: 'playlist', v: 1, name: p.name, tracks: byIds(p.ids).map((t) => ({ title: t.title, artist: tr(t.artist), album: t.album, dur: t.dur, url: t.url ?? null })) };
    try { await invoke('save_text', { path: f, text: JSON.stringify(data, null, 1) }); say(tr('บันทึกไฟล์เพลย์ลิสต์แล้ว ส่งไฟล์นี้ให้เพื่อนนำเข้าได้เลย')); } catch (e) { notice = tr(String(e)); }
  }
  const plWait = new Map<string, number>(); // ลิงก์ที่กำลังโหลด → เพลย์ลิสต์ที่ต้องใส่เมื่อโหลดเสร็จ
  const keyTA = (title: string, artist: string) => norm(title).replace(/\s+/g, '') + '|' + norm(artist).replace(/\s+/g, '');
  async function importListFile(f: string) {
    let text = '';
    try { text = await invoke<string>('load_text', { path: f }); } catch (e) { notice = tr(String(e)); return; }
    const base = f.replace(/[\\/][^\\/]*$/, '');
    const name = (f.split(/[\\/]/).pop() ?? 'Playlist').replace(/\.[^.]+$/, '');
    if (/\.m3u8?$/i.test(f)) {
      const paths = text.split(/\r?\n/).map((l) => l.trim()).filter((l) => l && !l.startsWith('#')).map((l) => (/^[a-z]:[\\/]|^[\\/]/i.test(l) ? l : `${base}\\${l}`));
      const ts = await importPaths(paths);
      const pid = newPlaylist(name);
      for (const t of ts) addToPlaylist(pid, t.id);
      fileMsg = tr('นำเข้าเพลย์ลิสต์ “{0}” {1} เพลง', name, ts.length);
      return;
    }
    if (/\.csv$/i.test(f)) return importSpotifyCsv(text, name);
    let d: { app?: string; kind?: string; name?: string; tracks?: { title: string; artist: string; url?: string | null }[] };
    try { d = JSON.parse(text); } catch { notice = tr('อ่านไฟล์นี้ไม่ได้'); return; }
    if (d.app !== 'MySound' || d.kind !== 'playlist' || !Array.isArray(d.tracks)) { notice = tr('ไฟล์นี้ไม่ใช่เพลย์ลิสต์ของ MySound'); return; }
    const pid = newPlaylist(d.name || name);
    let have = 0, queued = 0, lost = 0;
    for (const e of d.tracks) {
      const t = library.find((x) => (e.url && x.url && linkKey(x.url) === linkKey(e.url)) || keyTA(x.title, tr(x.artist)) === keyTA(e.title, e.artist));
      if (t) { addToPlaylist(pid, t.id); have++; continue; }
      if (e.url && isSupportedLink(e.url)) {
        const r = prepare(e.url);
        if ('d' in r) { plWait.set(linkKey(e.url), pid); queued++; continue; }
      }
      lost++;
    }
    if (queued) pump();
    fileMsg = tr('นำเข้า “{0}”: มีในคลัง {1} เพลง · กำลังโหลด {2} เพลง · หาไม่เจอ {3} เพลง', d.name || name, have, queued, lost);
  }
  async function pickListFile() {
    const f = await open({ filters: [{ name: tr('เพลย์ลิสต์'), extensions: ['m3u', 'm3u8', 'mysound', 'csv'] }] });
    if (typeof f === 'string') importListFile(f);
  }
  // Spotify (Exportify CSV): ค้นแต่ละเพลงบน YouTube แล้วเข้าคิวโหลด
  let csvJob = $state<{ done: number; total: number } | null>(null);
  function parseCsv(text: string): string[][] {
    const rows: string[][] = []; let row: string[] = [], cell = '', q = false;
    for (let i = 0; i < text.length; i++) {
      const c = text[i];
      if (q) { if (c === '"' && text[i + 1] === '"') { cell += '"'; i++; } else if (c === '"') q = false; else cell += c; }
      else if (c === '"') q = true;
      else if (c === ',') { row.push(cell); cell = ''; }
      else if (c === '\n' || c === '\r') { if (c === '\r' && text[i + 1] === '\n') i++; row.push(cell); rows.push(row); row = []; cell = ''; }
      else cell += c;
    }
    if (cell || row.length) { row.push(cell); rows.push(row); }
    return rows.filter((r) => r.some((x) => x.trim()));
  }
  async function importSpotifyCsv(text: string, name: string) {
    const rows = parseCsv(text);
    const h = rows.shift()?.map((x) => x.toLowerCase()) ?? [];
    const ti = h.findIndex((x) => /track name|^name$|title/.test(x)), ai = h.findIndex((x) => /artist/.test(x));
    if (ti < 0) { notice = tr('ไม่พบคอลัมน์ชื่อเพลงในไฟล์ CSV (ต้องมีคอลัมน์ “Track Name”)'); return; }
    const pid = newPlaylist(name);
    csvJob = { done: 0, total: rows.length };
    let have = 0, queued = 0, lost = 0;
    for (const r of rows) {
      const title = r[ti]?.trim() ?? '', artist = (ai >= 0 ? r[ai] : '').split(/[,;]/)[0].trim();
      csvJob.done++;
      if (!title) continue;
      const t = library.find((x) => keyTA(x.title, tr(x.artist)) === keyTA(title, artist));
      if (t) { addToPlaylist(pid, t.id); have++; continue; }
      try {
        const hits = await invoke<Hit[]>('search_online', { query: `${artist} ${title}`.trim(), source: 'youtube' });
        const best = hits.find((x) => !x.dur || x.dur < 900) ?? hits[0];
        if (best) {
          const pr = prepare(best.url);
          if ('d' in pr) { plWait.set(linkKey(best.url), pid); queued++; continue; }
          const ex = library.find((x) => x.url && linkKey(x.url) === linkKey(best.url));
          if (ex) { addToPlaylist(pid, ex.id); have++; continue; }
        }
      } catch { /* ค้นไม่สำเร็จ */ }
      lost++;
    }
    csvJob = null;
    if (queued) pump();
    fileMsg = tr('นำเข้า “{0}”: มีในคลัง {1} เพลง · กำลังโหลด {2} เพลง · หาไม่เจอ {3} เพลง', name, have, queued, lost);
  }
  // ---------- ธีมเป็นไฟล์ ----------
  async function exportTheme() {
    const f = await save({ defaultPath: 'MySound-theme.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (!f) return;
    const { bg: _bg, ...rest } = $state.snapshot(look);
    try { await invoke('save_text', { path: f, text: JSON.stringify({ app: 'MySound', kind: 'theme', v: 1, look: rest }, null, 1) }); say(tr('ส่งออกธีมแล้ว')); } catch (e) { notice = tr(String(e)); }
  }
  async function importTheme() {
    const f = await open({ filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (typeof f !== 'string') return;
    try {
      const d = JSON.parse(await invoke<string>('load_text', { path: f }));
      if (d?.app !== 'MySound' || d.kind !== 'theme') throw new Error(tr('ไฟล์นี้ไม่ใช่ธีมของ MySound'));
      look = normalizeLook({ ...d.look, bg: look.bg });
      say(tr('ใช้ธีมแล้ว'));
    } catch (e) { notice = tr(String(e instanceof Error ? e.message : e)); }
  }
  // ---------- นาฬิกาปลุก ----------
  let alarmFired = '';
  $effect(() => {
    if (!alarm.on) return;
    const h = setInterval(() => {
      const now = new Date(), hm = now.toTimeString().slice(0, 5), key = now.toDateString() + hm;
      if (hm !== alarm.time || alarmFired === key) return;
      alarmFired = key;
      const p = playlists.find((x) => String(x.id) === alarm.pl);
      const list = p ? byIds(p.ids) : library;
      if (!list.length) return;
      const target = volume;
      if (alarm.ramp) { volume = 5; }
      play(list[0], list);
      getCurrentWindow().show().catch(() => {});
      if (alarm.ramp) {
        const t0 = Date.now();
        const r = setInterval(() => { const k = Math.min(1, (Date.now() - t0) / 60000); volume = Math.round(5 + (target - 5) * k); if (k >= 1) clearInterval(r); }, 1000);
      }
    }, 15000);
    return () => clearInterval(h);
  });
  // ---------- ติดตามช่อง/เพลย์ลิสต์ YouTube ----------
  let followUrl = $state('');
  let followMsg = $state('');
  async function addFollow() {
    const u = followUrl.trim();
    if (!/^https:\/\/(www\.|m\.)?(youtube\.com|soundcloud\.com)\//.test(u)) return void (followMsg = tr('ใส่ลิงก์ช่องหรือเพลย์ลิสต์ของ YouTube/SoundCloud'));
    followMsg = tr('กำลังตรวจ…');
    try {
      const i = await invoke<{ title: string; entries: { url: string }[] }>('yt_info', { url: u, max: 30, cookies: ytCookies });
      follows.push({ url: u, name: i.title || u, seen: i.entries.map((e) => linkKey(e.url)), at: Date.now() });
      followUrl = '';
      followMsg = tr('ติดตาม “{0}” แล้ว จะโหลดเฉพาะเพลงใหม่ที่ลงหลังจากนี้', i.title || u);
    } catch (e) { followMsg = tr(String(e)); }
  }
  async function checkFollows(manual = false) {
    let n = 0;
    for (const f of follows) {
      try {
        const i = await invoke<{ entries: { url: string }[] }>('yt_info', { url: f.url, max: 15, cookies: ytCookies });
        for (const e of i.entries) {
          const k = linkKey(e.url);
          if (f.seen.includes(k)) continue;
          f.seen.push(k);
          if (!library.some((t) => t.url && linkKey(t.url) === k) && 'd' in prepare(e.url)) n++;
        }
        if (f.seen.length > 500) f.seen = f.seen.slice(-500);
        f.at = Date.now();
      } catch { /* เน็ตหลุด: ไว้ตรวจรอบหน้า */ }
    }
    if (n) { pump(); say(tr('พบเพลงใหม่ {0} เพลงจากช่องที่ติดตาม กำลังโหลด', n)); }
    else if (manual) followMsg = tr('ยังไม่มีเพลงใหม่');
  }
  $effect(() => {
    if (!loaded || !follows.length) return;
    const due = () => follows.some((f) => Date.now() - f.at > 6 * 3600e3);
    const t0 = setTimeout(() => due() && checkFollows(), 30000);
    const h = setInterval(() => due() && checkFollows(), 30 * 60e3);
    return () => { clearTimeout(t0); clearInterval(h); };
  });
  // ---------- แยกมิกซ์ตามบท / ตัดช่วงเป็นเพลงใหม่ ----------
  let cutBusy = $state(false);
  let cutMsg = $state('');
  let clipA = $state(0), clipB = $state(0);
  function newTrackFrom(src: Track, path: string, title: string, dur: number): Track {
    const t: Track = { id: Date.now() + seq++, src: src.src, added: today(), last: today(), plays: 0, title, artist: src.artist, album: src.album || src.title, dur: Math.round(dur), path, coverUrl: src.coverUrl };
    library.unshift(t);
    return library[0];
  }
  async function splitChapters() {
    if (!editing?.url || cutBusy) return;
    cutBusy = true; cutMsg = tr('กำลังอ่านบทของวิดีโอ…');
    try {
      const i = await invoke<{ chapters: { title: string; start: number; end: number }[] }>('yt_info', { url: editing.url, max: 1, cookies: ytCookies });
      if (i.chapters.length < 2) throw new Error(tr('วิดีโอนี้ไม่มีการแบ่งบท'));
      checkpoint();
      const src = editing, pid = newPlaylist(src.title);
      for (const [k, c] of i.chapters.entries()) {
        cutMsg = tr('กำลังตัด {0}/{1}…', k + 1, i.chapters.length);
        const end = Math.min(c.end, src.dur || c.end);
        const path = await invoke<string>('cut_audio', { path: src.path, start: c.start, end });
        addToPlaylist(pid, newTrackFrom(src, path, c.title || `${src.title} ${k + 1}`, end - c.start).id);
      }
      cutMsg = tr('แยกเป็น {0} เพลงแล้ว และใส่ไว้ในเพลย์ลิสต์ “{1}”', i.chapters.length, src.title);
    } catch (e) { cutMsg = tr(String(e instanceof Error ? e.message : e)); }
    cutBusy = false;
  }
  async function saveClip() {
    if (!editing || cutBusy) return;
    if (!(clipB > clipA + 0.5)) return void (cutMsg = tr('จุดจบต้องอยู่หลังจุดเริ่มอย่างน้อยครึ่งวินาที'));
    cutBusy = true; cutMsg = tr('กำลังตัด…');
    try {
      const path = await invoke<string>('cut_audio', { path: editing.path, start: clipA, end: clipB });
      checkpoint();
      newTrackFrom(editing, path, `${editing.title} (${fmt(clipA)}–${fmt(clipB)})`, clipB - clipA);
      cutMsg = tr('บันทึกเป็นเพลงใหม่แล้ว');
    } catch (e) { cutMsg = tr(String(e)); }
    cutBusy = false;
  }
  // ---------- แปลงเป็น MP3 ----------
  let mp3Busy = $state(false);
  async function toMp3(ts: Track[]) {
    const dir = await open({ directory: true, title: tr('เลือกโฟลเดอร์ปลายทาง') });
    if (typeof dir !== 'string') return;
    mp3Busy = true; say(tr('กำลังแปลง {0} เพลงเป็น MP3…', ts.length));
    try {
      const n = await invoke<number>('convert_mp3', { dest: dir, items: ts.map((t) => ({ path: t.path, title: t.title, artist: tr(t.artist), album: t.album ?? '', cover: null })) });
      say(tr('แปลงเสร็จ {0} จาก {1} เพลง', n, ts.length));
    } catch (e) { notice = tr(String(e)); }
    mp3Busy = false;
  }
  // ---------- การ์ดแชร์ (รูป PNG): หน้าต่างเลือกสไตล์/บรรทัด อยู่ใน ShareCard.svelte ----------
  let shareOpen = $state<{ lines: string[]; first: number } | null>(null);
  async function shareCard() {
    const t = current;
    if (!t) return;
    await loadLyrics(t);
    const ls = ly.lines.length ? ly.lines.map((l) => l.text.trim()).filter(Boolean) : ly.plain.split('\n').map((x) => x.trim()).filter((x) => x && !/^\[.*\]$/.test(x));
    const at = ly.lines.length && lyIdx >= 0 ? ls.indexOf(ly.lines[lyIdx].text.trim()) : -1;
    shareOpen = { lines: ls, first: Math.max(0, at) };
  }
  const safeFile = (x: string) => x.replace(/[<>:"/\\|?*]+/g, '_').slice(0, 80) || 'MySound';
  // ---------- Last.fm ----------
  let lfmMsg = $state('');
  let lfmToken = '';
  async function lfmConnect() {
    if (!lfm.key.trim() || !lfm.secret.trim()) return void (lfmMsg = tr('ใส่ API key และ secret ก่อน (สร้างได้ที่ last.fm/api/account/create)'));
    try {
      const [tok, url] = await invoke<[string, string]>('lastfm_token', { key: lfm.key.trim(), secret: lfm.secret.trim() });
      lfmToken = tok;
      await invoke('open_url', { url });
      lfmMsg = tr('กด “Yes, allow access” ในเบราว์เซอร์ แล้วกลับมากด “ยืนยัน”');
    } catch (e) { lfmMsg = tr(String(e)); }
  }
  async function lfmConfirm() {
    try {
      const [sk, user] = await invoke<[string, string]>('lastfm_session', { key: lfm.key.trim(), secret: lfm.secret.trim(), token: lfmToken });
      lfm.sk = sk; lfm.user = user; lfm.on = true;
      lfmMsg = tr('เชื่อมต่อ Last.fm แล้ว ({0})', user);
    } catch (e) { lfmMsg = tr(String(e)); }
  }
  let lfmStart = 0, lfmDone = 0, lfmNow = 0;
  $effect(() => {
    const t = current;
    if (!t || !lfm.on || !lfm.sk) return;
    if (playing && lfmNow !== t.id) {
      lfmNow = t.id; lfmStart = Math.floor(Date.now() / 1000); lfmDone = 0;
      invoke('lastfm_track', { key: lfm.key, secret: lfm.secret, sk: lfm.sk, artist: tr(t.artist), track: t.title, album: t.album ?? '', ts: lfmStart, dur: Math.round(t.dur), now: true }).catch(() => {});
    }
    // กฎของ Last.fm: เพลงยาวกว่า 30 วินาที และฟังครบครึ่งเพลงหรือ 4 นาที
    if (lfmDone !== t.id && t.dur > 30 && pos >= Math.min(t.dur / 2, 240)) {
      lfmDone = t.id;
      invoke('lastfm_track', { key: lfm.key, secret: lfm.secret, sk: lfm.sk, artist: tr(t.artist), track: t.title, album: t.album ?? '', ts: lfmStart || Math.floor(Date.now() / 1000), dur: Math.round(t.dur), now: false }).catch(() => {});
    }
  });
  // ---------- รีโมทผ่านมือถือ ----------
  let remoteMsg = $state('');
  async function setRemote(on: boolean) {
    remoteMsg = '';
    if (on) {
      try { remoteInfo = await invoke<{ urls: string[]; pin: string }>('remote_start', { port: 8765 }); remoteOn = true; }
      catch (e) { remoteOn = false; remoteMsg = tr(String(e)); }
    } else { await invoke('remote_stop').catch(() => {}); remoteOn = false; remoteInfo = null; }
  }
  let tracksJson = '';
  $effect(() => {
    if (!remoteOn) return;
    void library.length;
    tracksJson = JSON.stringify(library.slice(0, 3000).map((t) => ({ i: t.id, t: t.title, a: tr(t.artist) })));
  });
  $effect(() => {
    if (!remoteOn) return;
    const h = setInterval(() => {
      const s0 = `{"title":${JSON.stringify(current?.title ?? '')},"artist":${JSON.stringify(current ? tr(current.artist) : '')},"id":${current?.id ?? 0},"playing":${playing},"pos":${Math.floor(pos)},"dur":${current?.dur ?? 0},"vol":${volume},"accent":${JSON.stringify(themeVars['--accent'] ?? '')},"tracks":${tracksJson || '[]'}}`;
      invoke('remote_push', { json: s0 }).catch(() => {});
    }, 1000);
    return () => clearInterval(h);
  });
  // ---------- UI: รายการยาว (แสดงเฉพาะแถวที่เห็น), ลากจัดลำดับ, แถบเวลา, หน้า “กำลังเล่น” ----------
  function measureTop(node: HTMLElement) {
    const upd = () => { if (mainEl) rowsTop = node.getBoundingClientRect().top - mainEl.getBoundingClientRect().top + mainEl.scrollTop; };
    upd();
    const ro = new ResizeObserver(upd);
    ro.observe(node); if (mainEl) ro.observe(mainEl);
    return { destroy: () => ro.disconnect() };
  }
  const VIRT_MIN = 250; // เกินนี้ค่อยแสดงเฉพาะแถวที่อยู่บนจอ
  function winRange(n: number): [number, number] {
    if (n < VIRT_MIN) return [0, n];
    const h = look.row, a = Math.max(0, Math.floor((scrollY - rowsTop) / h) - 15);
    return [a, Math.min(n, a + Math.ceil(viewH / h) + 30)];
  }
  // ลากเพลง: ลากแถวไปวางในเพลย์ลิสต์ที่แถบซ้าย หรือจัดลำดับในเพลย์ลิสต์ (ใช้ pointer ไม่ใช้ HTML5 drag เพราะ WebView จองไว้รับไฟล์)
  let dragStart: { x: number; y: number; t: Track; list: Track[]; plId?: number } | null = null, justDragged = false;
  function rowDown(e: PointerEvent, t: Track, list: Track[], plId?: number) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button,input')) return;
    dragStart = { x: e.clientX, y: e.clientY, t, list, plId };
  }
  function onPointerMove(e: PointerEvent) {
    if (seekHover && !(e.target as HTMLElement)?.closest?.('.seekwrap')) seekHover = null;
    if (!dragStart) return;
    if (!drag) {
      if (Math.hypot(e.clientX - dragStart.x, e.clientY - dragStart.y) < 7) return;
      const ids = selIds.includes(dragStart.t.id) && selIds.length > 1 ? [...selIds] : [dragStart.t.id];
      drag = { ids, x: e.clientX, y: e.clientY, label: ids.length > 1 ? tr('{0} เพลง', ids.length) : dragStart.t.title, over: null, overPl: null, plId: dragStart.plId };
      document.body.classList.add('dragging');
    }
    drag.x = e.clientX; drag.y = e.clientY;
    const el = document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null;
    const plEl = el?.closest<HTMLElement>('[data-pl]');
    drag.overPl = plEl ? +plEl.dataset.pl! : null;
    const rowEl = drag.plId && !sortKey ? el?.closest<HTMLElement>('.row[data-idx]') : null;
    if (rowEl) {
      const r = rowEl.getBoundingClientRect(), i = +rowEl.dataset.idx!;
      drag.over = e.clientY > r.top + r.height / 2 ? i + 1 : i;
    } else drag.over = null;
  }
  function onPointerUp() {
    if (drag) {
      justDragged = true;
      setTimeout(() => (justDragged = false), 0);
      if (drag.overPl !== null) {
        const p = playlists.find((x) => x.id === drag!.overPl);
        if (p) {
          checkpoint();
          const add = drag.ids.filter((id) => !p.ids.includes(id));
          p.ids.push(...add);
          say(add.length ? tr('เพิ่ม {0} เพลงลง “{1}”', add.length, p.name) : tr('เพลงนี้อยู่ใน “{0}” แล้ว', p.name));
        }
      } else if (drag.over !== null && drag.plId) {
        const p = playlists.find((x) => x.id === drag!.plId);
        if (p) {
          checkpoint();
          const moving = drag.ids.filter((id) => p.ids.includes(id));
          const before = p.ids.slice(0, drag.over).filter((id) => !moving.includes(id)).length;
          const rest = p.ids.filter((id) => !moving.includes(id));
          rest.splice(before, 0, ...moving);
          flipNow = true; setTimeout(() => (flipNow = false), 50);
          p.ids = rest;
        }
      }
      document.body.classList.remove('dragging');
    }
    drag = null;
    dragStart = null;
  }
  // แถบเวลา: ชี้แล้วเห็นเวลา
  function seekMove(e: PointerEvent) {
    if (!current?.dur) return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const k = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    seekHover = { x: e.clientX - r.left, t: k * current.dur };
  }
  // เปิดหน้า "กำลังเล่น" เต็มหน้าต่าง (ใช้ View Transition ถ้า WebView รองรับ ให้ปกขยายต่อเนื่อง)
  function setNowFull(v: boolean) {
    const d = document as Document & { startViewTransition?: (f: () => void) => unknown };
    if (d.startViewTransition && !matchMedia('(prefers-reduced-motion: reduce)').matches) d.startViewTransition(() => { nowFull = v; return tick(); });
    else nowFull = v;
  }
  // หัวเพลย์ลิสต์: ปกรวมจาก 4 เพลงแรกที่มีปก
  const mosaic = (ts: Track[]) => ts.filter((t) => t.cover).slice(0, 4);
  const totalDur = (ts: Track[]) => {
    const s0 = ts.reduce((a, t) => a + t.dur, 0), h = Math.floor(s0 / 3600), m = Math.round((s0 % 3600) / 60);
    return h ? tr('{0} ชม. {1} นาที', h, m) : tr('{0} นาที', m);
  };
  function shufflePlay(ts: Track[]) {
    if (!ts.length) return;
    shuffle = true;
    play(ts[Math.floor(Math.random() * ts.length)], ts);
  }
  // ลากขอบแผงเพื่อปรับความกว้าง
  function resizer(e: PointerEvent, side: 'sidebar' | 'right') {
    const x0 = e.clientX, w0 = look[side];
    const mv = (ev: PointerEvent) => {
      const d = side === 'sidebar' ? ev.clientX - x0 : x0 - ev.clientX;
      look[side] = side === 'sidebar' ? Math.max(180, Math.min(320, w0 + d)) : Math.max(240, Math.min(440, w0 + d));
    };
    const up = () => { removeEventListener('pointermove', mv); removeEventListener('pointerup', up); document.body.classList.remove('resizing'); };
    addEventListener('pointermove', mv); addEventListener('pointerup', up);
    document.body.classList.add('resizing');
  }
  function resizeKey(e: KeyboardEvent, side: 'sidebar' | 'right') {
    const d = e.key === 'ArrowLeft' ? -10 : e.key === 'ArrowRight' ? 10 : 0;
    if (!d) return;
    e.preventDefault();
    const s0 = side === 'sidebar' ? d : -d;
    look[side] = side === 'sidebar' ? Math.max(180, Math.min(320, look.sidebar + s0)) : Math.max(240, Math.min(440, look.right + s0));
  }
  // เมนูคลิกขวา: ลูกศรขึ้นลงเลื่อน, → เปิดเมนูย่อยเพลย์ลิสต์, ← ปิด
  let subOpen = $state(false);
  function ctxKey(e: KeyboardEvent) {
    const root = e.currentTarget as HTMLElement;
    const items = [...root.querySelectorAll<HTMLButtonElement>('[role=menuitem]')].filter((b) => b.offsetParent);
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      items[(i + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length]?.focus();
    } else if (e.key === 'ArrowRight' && (document.activeElement as HTMLElement)?.dataset.sub) {
      e.preventDefault(); subOpen = true; tick().then(() => root.querySelector<HTMLButtonElement>('.sub [role=menuitem]')?.focus());
    } else if (e.key === 'ArrowLeft' && subOpen) {
      e.preventDefault(); subOpen = false; tick().then(() => root.querySelector<HTMLButtonElement>('[data-sub]')?.focus());
    } else if (e.key === 'Escape') { ctx = null; }
  }
  function ctxMount(node: HTMLElement) {
    subOpen = false;
    // ล้นจอ: ขยับขึ้น/ซ้าย และให้อนิเมชันขยายจากมุมที่ใกล้เมาส์
    const r = node.getBoundingClientRect();
    let ox = 'left', oy = 'top';
    if (r.bottom > innerHeight - 8) { node.style.top = `${Math.max(8, innerHeight - r.height - 8)}px`; oy = 'bottom'; }
    if (r.right > innerWidth - 8) { node.style.left = `${Math.max(8, innerWidth - r.width - 8)}px`; ox = 'right'; }
    node.style.transformOrigin = `${oy} ${ox}`;
    node.querySelector<HTMLButtonElement>('[role=menuitem]')?.focus();
  }
  let prevVol = 70;
  function toggleMute() {
    if (volume > 0) { prevVol = volume; volume = 0; } else volume = prevVol || 70;
  }
  function wheelVol(e: WheelEvent) {
    e.preventDefault();
    volume = Math.max(0, Math.min(100, volume + (e.deltaY < 0 ? 5 : -5)));
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


  // ---------- Discord Rich Presence: State Machine ----------
  // สถานะที่ต้องการ (desired) = off | paused | playing(+เพลง+บรรทัดเนื้อเพลง) อ่านจาก <audio> จริง ไม่เชื่อธง `playing`
  // สถานะที่ส่งไปล่าสุด (sent) เก็บเป็น key แล้ว reconcile(): ถ้า desired ≠ sent ค่อยส่ง, ส่งทีละครั้ง (inflight)
  // ถ้าระหว่างส่งมีอะไรเปลี่ยนก็ทำซ้ำอีกรอบ + heartbeat ทุก 10 วิ ซ่อมกรณี Discord ทิ้งอัปเดต
  let dSent = '', dBusy = false, dDirty = false;
  function dDesired(): { key: string; t: Track | null; mode: 'off' | 'paused' | 'playing'; line: string } {
    const t = current;
    if (!loaded || !discord || !t) return { key: 'off', t: null, mode: 'off', line: '' };
    const mode = audio && !audio.paused && !audio.ended ? 'playing' : 'paused';
    const line = discordLy && mode === 'playing' ? dLine : '';
    return { key: `${t.id}|${mode}|${line}|${seeks}`, t, mode, line };
  }
  async function dReconcile() {
    if (dBusy) { dDirty = true; return; }
    const d = dDesired();
    if (d.key === dSent) return;
    dBusy = true;
    try {
      if (d.mode === 'off' || !d.t) {
        await invoke('discord_clear').catch(() => {});
        discordMsg = '';
      } else {
        await invoke('discord_update', { title: d.t.title, artist: tr(d.t.artist), playing: d.mode === 'playing', pos: audio?.currentTime ?? 0, dur: d.t.dur, cover: d.t.coverUrl ?? null, line: d.line || null, url: d.t.url ?? null });
        discordMsg = tr('เชื่อมต่อ Discord แล้ว');
      }
      dSent = d.key;
    } catch (e) {
      dSent = ''; // ส่งไม่สำเร็จ: heartbeat จะลองใหม่
      discordMsg = tr(String(e));
    } finally {
      dBusy = false;
      if (dDirty) { dDirty = false; dReconcile(); }
    }
  }
  $effect(() => {
    void [loaded, current?.id, discord, playing, discordLy, dLine, seeks];
    const h = setTimeout(dReconcile, 400); // รวมเหตุการณ์ที่ซ้อนกัน (เช่น pause→play ตอนเปลี่ยนเพลง) ให้เหลือสถานะสุดท้าย
    return () => clearTimeout(h);
  });
  $effect(() => {
    const h = setInterval(dReconcile, 10000);
    return () => clearInterval(h);
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
  const cycling = $derived(look.accentMode === 'cycle' && look.accentStops.length >= 2);
  // ตอนวนสี ค่าคงที่ใช้สีแรกของชุด (รีโมต/เนื้อเพลงลอย/การ์ดแชร์) ส่วนบนหน้าจอให้ลูปด้านล่างเปลี่ยนเอง
  const themeVars = $derived(lookVars(cycling ? { ...look, accent: look.accentStops[0] } : coverAccent && accentOv ? { ...look, accent: accentOv } : look));
  $effect(() => void (document.documentElement.lang = i18n.lang));
  $effect(() => {
    const r = document.documentElement;
    const skip = cycling; // ให้ลูปวนสีเป็นคนตั้ง --accent เอง ไม่งั้นจะกระพริบกลับสีแรก
    for (const [k, v] of Object.entries(themeVars)) if (!(skip && (k === '--accent' || k === '--on-accent'))) r.style.setProperty(k, v);
    r.style.fontSize = `${look.font}%`;
    r.style.colorScheme = isLight(look) ? 'light' : 'dark';
  });
  // ลากหน้าต่างจากแถบบน (ที่ว่างหรือข้อความ ยกเว้นช่องพิมพ์/ปุ่ม) ดับเบิลคลิก = ขยาย/คืนขนาด
  function navDown(e: MouseEvent) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('input,button,select,textarea,a')) return;
    e.preventDefault();
    const w = getCurrentWindow();
    if (e.detail === 2) w.toggleMaximize(); else w.startDragging().catch(() => {});
  }

  // ---------- แอนิเมชัน ----------
  let reduced = $state(matchMedia('(prefers-reduced-motion: reduce)').matches);
  $effect(() => {
    const mq = matchMedia('(prefers-reduced-motion: reduce)');
    const f = () => (reduced = mq.matches);
    mq.addEventListener('change', f);
    return () => mq.removeEventListener('change', f);
  });
  const moving = $derived(look.motion && !reduced);
  $effect(() => {
    const r = document.documentElement;
    r.classList.toggle('nomo', !moving);
    r.classList.toggle('acc-ease', moving && !cycling); // ตอนวนสีไม่ต้องไล่ซ้อน ไม่งั้นสีจะตามไม่ทัน
  });
  const EASE = 'cubic-bezier(0.23, 1, 0.32, 1)';
  const easeOut = (t: number) => 1 - (1 - t) ** 4;
  // ปก/ชื่อเพลงตอนเปลี่ยนเพลง: อันใหม่ชัดขึ้นจากเบลอ อันเก่าจางออก (ซ้อนกันเพราะเป็น absolute)
  const coverIn = (_: Element) => ({ duration: moving ? 320 : 0, easing: easeOut, css: (t: number) => `opacity:${t};filter:blur(${(1 - t) * 6}px);transform:scale(${0.94 + t * 0.06})` });
  const fadeOut = (_: Element) => ({ duration: moving ? 220 : 0, css: (t: number) => `opacity:${t}` });
  const textIn = (_: Element) => ({ duration: moving ? 260 : 0, easing: easeOut, css: (t: number) => `opacity:${t};filter:blur(${(1 - t) * 3}px);transform:translateY(${(1 - t) * 6}px)` });
  // toast เข้าและออกทางเดียวกัน (จากล่าง) เพื่อให้รู้ว่ามันไปไหน
  const toastT = (_: Element, { inn = true } = {}) => ({ duration: moving ? (inn ? 300 : 180) : 0, easing: easeOut, css: (t: number) => `opacity:${t};transform:translate(-50%, ${(1 - t) * 18}px) scale(${0.96 + t * 0.04})` });
  // แถวที่เพิ่ม/ลบ: ยุบ/ขยายความสูง แต่ถ้าเปลี่ยนทีละเยอะ (เช่น พิมพ์ค้นหา) หรือรายการยาวที่เลื่อนแบบเสมือน ให้ข้ามไปเลย
  let rowBurst = 0;
  const rowHeight = (node: Element, { n = 0 } = {}) => {
    rowBurst++;
    queueMicrotask(() => (rowBurst = 0));
    return () => {
      if (!moving || n >= VIRT_MIN || rowBurst > 6) return { duration: 0 };
      const h = (node as HTMLElement).offsetHeight;
      return { duration: 200, easing: easeOut, css: (t: number) => `height:${t * h}px;opacity:${t};overflow:hidden` };
    };
  };
  // เปิดหน้ารายการ: 12 แถวแรกขึ้นแบบสเตเกอร์ (ครั้งเดียวตอนเปิดหน้า ไม่ใช่ตอนเลื่อน)
  let stagger = $state(false);
  $effect(() => {
    void view;
    stagger = true;
    const h = setTimeout(() => (stagger = false), 700);
    return () => clearTimeout(h);
  });
  // หลังวางเพลงที่ลากจัดลำดับ แถวไหลเข้าที่ (เฉพาะตอนวาง ไม่ใช่ตอนเรียงหรือค้นหา)
  let flipNow = false;
  const rowFlip = (node: Element, { from, to }: { from: DOMRect; to: DOMRect }) => {
    const dy = from.top - to.top;
    if (!flipNow || !moving || !dy) return { duration: 0 };
    return { duration: 240, easing: easeOut, css: (t: number) => `transform:translateY(${(1 - t) * dy}px)` };
  };
  // หน้าต่าง (dialog) ขยายออกจากจุดที่กด ถ้าเปิดด้วยคีย์บอร์ดค่อยขยายจากกลาง
  let lastPress = { x: 0, y: 0, t: 0 };
  $effect(() => {
    const f = (e: PointerEvent) => (lastPress = { x: e.clientX, y: e.clientY, t: performance.now() });
    addEventListener('pointerdown', f, true);
    return () => removeEventListener('pointerdown', f, true);
  });
  function openDlg(d: HTMLDialogElement | undefined | null) {
    if (!d) return;
    if (!d.open) d.showModal();
    if (!moving) return;
    const r = d.getBoundingClientRect(), fresh = performance.now() - lastPress.t < 1500;
    d.style.transformOrigin = fresh ? `${lastPress.x - r.left}px ${lastPress.y - r.top}px` : 'center';
    d.animate([{ opacity: 0, transform: 'scale(0.9)' }, { opacity: 1, transform: 'none' }], { duration: 240, easing: EASE });
  }
  // หัวใจเด้งตอนกดถูกใจ (ไม่เด้งตอนเลิกถูกใจ)
  function heartPop(el: Element | null | undefined, on: boolean) {
    const svg = el?.querySelector('svg');
    if (!svg || !on || !moving) return;
    svg.animate([{ transform: 'scale(1)' }, { transform: 'scale(1.35)', offset: 0.35 }, { transform: 'scale(0.92)', offset: 0.7 }, { transform: 'scale(1)' }], { duration: 420, easing: 'ease-out' });
  }
  const favClick = (e: MouseEvent, t: Track) => { toggleFav(t); heartPop(e.currentTarget as Element, !!t.fav); };

  // ---------- ตัวละคร ----------
  const pet = $derived(pets.find((x) => x.id === petId) ?? null);
  const PET_SLOTS = [['idle', 'อยู่เฉยๆ'], ['hover', 'เอาเมาส์ชี้'], ['press', 'คลิก']] as const;
  const petMissing = (p: Pet | null) => (p ? PET_SLOTS.filter(([k]) => !p[k]).map(([, l]) => tr(l)) : []);
  const petReady = $derived(!!pet && petMissing(pet).length === 0);
  function addPet() {
    const p: Pet = { id: crypto.randomUUID(), name: tr('ตัวละคร {0}', pets.length + 1), idle: '', hover: '', press: '', size: 200, locked: false };
    pets.push(p);
    petId = p.id;
  }
  async function pickSprite(k: 'idle' | 'hover' | 'press') {
    if (!pet) return;
    const f = await open({ filters: [{ name: tr('รูปภาพ'), extensions: ['png', 'gif', 'webp'] }] });
    if (typeof f !== 'string') return;
    petMsg = '';
    try { pet[k] = await invoke<string>('pet_import', { src: f, old: pet[k] || null }); } catch (e) { petMsg = tr(String(e)); }
  }
  function delPet(p: Pet) {
    invoke('delete_files', { paths: [p.idle, p.hover, p.press].filter(Boolean) }).catch(() => {});
    pets = pets.filter((x) => x !== p);
    if (petId === p.id) { petId = pets[0]?.id ?? null; if (!pets.length) petOn = false; }
  }
  // ความดังของเพลง (เสียงทุ้ม) ส่งให้ตัวละครราว 30 ครั้ง/วินาที เฉพาะตอนเปิดโหมดเด้งและเพลงกำลังเล่น
  $effect(() => {
    if (!petOn || !petReady || !petBounce || !playing || !moving) return;
    attachViz();
    const bins = new Uint8Array(64);
    let last = -1;
    const t = setInterval(() => {
      if (!analyser) return;
      analyser.getByteFrequencyData(bins);
      let sum = 0;
      for (let i = 1; i < 9; i++) sum += bins[i];
      const lv = Math.round(Math.min(1, (sum / 8 / 255) * 1.35) * 100) / 100;
      if (lv !== last) { last = lv; emit('pet-level', lv).catch(() => {}); }
    }, 33);
    return () => clearInterval(t);
  });
  const petCfg = () => pet && { idle: pet.idle, hover: pet.hover, press: pet.press, size: pet.size, locked: pet.locked, x: pet.x, y: pet.y, bounce: petBounce, life: petLife, ly: petLy, line: petLy && playing && ly.status === 'ready' ? curLine : '', playing, bpm: current?.bpm ?? 0, lang: i18n.lang, motion: moving };
  // เปิด/ปิดหน้าต่างตามสวิตช์ (ใส่รูปไม่ครบ = ไม่เปิด) และส่งค่าล่าสุดให้ทุกครั้งที่เปลี่ยน
  let petWinId = '';
  $effect(() => {
    const want = petOn && petReady ? pet!.id : '';
    if (want === petWinId) return;
    petWinId = want;
    (async () => {
      const ex = await WebviewWindow.getByLabel('pet');
      if (ex) await ex.close().catch(() => {});
      if (!want) return;
      if (ex) await new Promise((r) => setTimeout(r, 150)); // รอหน้าต่างเก่าปิดก่อนสร้างใหม่ (สลับตัวละคร)
      new WebviewWindow('pet', { url: '/', title: 'MySound Character', width: 200, height: 200, transparent: true, decorations: false, alwaysOnTop: true, skipTaskbar: true, shadow: false, resizable: false, focus: false, visible: false });
    })().catch((e) => (petMsg = tr(String(e))));
  });
  $effect(() => {
    const c = petCfg();
    if (petWinId && c) emit('pet-cfg', $state.snapshot(c)).catch(() => {});
  });
  onMount(() => {
    const subs = [
      listen('pet-ready', () => { const c = petCfg(); if (c) emit('pet-cfg', $state.snapshot(c)); }),
      listen<{ size?: number; x?: number; y?: number; locked?: boolean; side?: 'left' | 'right'; hide?: boolean }>('pet-set', (e) => {
        const p = pet, v = e.payload;
        if (!p) return;
        if (v.size) p.size = v.size;
        if (v.x != null && v.y != null) { p.x = v.x; p.y = v.y; }
        if (v.locked != null) p.locked = v.locked;
        if (v.side) emit('pet-snap', v.side);
        if (v.hide) petOn = false;
      })
    ];
    return () => subs.forEach((s) => s.then((f) => f()));
  });

  // ---------- Spotify ----------
  $effect(() => { invoke<string | null>('spotify_status').then((u) => (spUser = u)).catch(() => {}); });
  async function spConnect() {
    spBusy = true;
    spMsg = tr('เปิดเบราว์เซอร์แล้ว ล็อกอิน Spotify แล้วกด Agree…');
    try { spUser = await invoke<string>('spotify_login', { clientId: spCid }); spMsg = ''; say(tr('เชื่อม Spotify กับ {0} แล้ว', spUser)); }
    catch (e) { spMsg = tr(String(e)); }
    finally { spBusy = false; }
  }
  async function spDisconnect() { await invoke('spotify_logout').catch(() => {}); spUser = null; spPls = null; }
  async function spLoadPls() {
    spMsg = '';
    try { spPls = await invoke('spotify_playlists'); } catch (e) { spMsg = tr(String(e)); }
  }
  const nkey = (s: string) => s.normalize('NFKC').toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '');
  function spPut(plId: number, id: number) { const p = playlists.find((x) => x.id === plId); if (p && !p.ids.includes(id)) p.ids.push(id); }
  /** ส่งผลที่เลือกเข้าคิวดาวน์โหลด (ถ้ามีในคลังแล้วแค่ใส่เพลย์ลิสต์) */
  function spQueue(it: SpItem, h: SpHit) {
    const imp = spImp;
    if (!imp) return;
    const have = library.find((x) => x.url && linkKey(x.url) === linkKey(h.url));
    if (have) { spPut(imp.plId, have.id); it.st = 'have'; return; }
    const r = prepare(h.url);
    if ('error' in r) { it.st = 'none'; return; }
    r.d.title = it.t.artist ? `${it.t.artist} - ${it.t.title}` : it.t.title;
    if (it.t.artist) r.d.meta = { title: it.t.title, artist: it.t.artist, album: it.t.album }; // ไม่มีศิลปิน = ใช้ชื่อจากคลิปตามเดิม
    plWait.set(linkKey(h.url), imp.plId);
    it.st = 'ok';
    pump();
  }
  async function spImport(src: string) {
    if (spImp && !spImp.done) return say(tr('กำลังนำเข้าอยู่ รอให้เสร็จก่อน'));
    spMsg = ''; err = '';
    view = 'download';
    let l: { name: string; tracks: SpTrack[] };
    try { l = await invoke('spotify_tracks', { src }); }
    catch (e) { err = tr(String(e)); return; }
    if (!l.tracks.length) { err = tr('ไม่มีเพลงในรายการนี้'); return; }
    await importTracks(src === 'liked' ? tr('เพลงที่ถูกใจ (Spotify)') : l.name, l.tracks);
  }
  /** วางรายชื่อเพลงเอง (ไม่ต้องล็อกอิน) */
  function importPasted() {
    const ts = parseList(pasteList);
    if (!ts.length) return void (err = tr('วางรายชื่อเพลงก่อน บรรทัดละเพลง เช่น “ศิลปิน - ชื่อเพลง”'));
    if (spImp && !spImp.done) return say(tr('กำลังนำเข้าอยู่ รอให้เสร็จก่อน'));
    pasteList = '';
    importTracks(pasteName.trim() || tr('นำเข้า {0}', new Date().toLocaleDateString(i18n.lang === 'th' ? 'th-TH' : 'en-GB')), ts);
  }
  /** หาเพลงแต่ละเพลงบน YouTube แล้วส่งเข้าคิวโหลด ใส่ทุกเพลงไว้ในเพลย์ลิสต์ชื่อ name */
  async function importTracks(name: string, tracks: SpTrack[]) {
    const plId = playlists.find((p) => p.name === name)?.id ?? newPlaylist(name);
    spImp = { name, plId, items: tracks.map((t) => ({ t, st: 'wait' })), stop: false, done: false };
    const imp = spImp;
    let next = 0;
    const worker = async () => {
      while (!imp.stop && next < imp.items.length) {
        const it = imp.items[next++];
        // มีในคลังแล้ว (ชื่อ+ศิลปินตรง ความยาวใกล้กัน) ไม่ต้องค้น
        const have = library.find((x) => nkey(x.title) === nkey(it.t.title) && nkey(x.artist).includes(nkey(it.t.artist.split(',')[0])) && (!it.t.dur || Math.abs(x.dur - it.t.dur) <= 4));
        if (have) { spPut(imp.plId, have.id); it.st = 'have'; continue; }
        it.st = 'search';
        let hits: SpHit[] = [];
        try { hits = await invoke('search_online', { query: spQuery(it.t), source: 'youtube' }); } catch { /* ค้นไม่ได้ = ให้เลือกเอง */ }
        const r = spPick(it.t, hits);
        if (r.sure && r.best) spQueue(it, r.best);
        else { it.alts = r.alts; it.st = r.alts.length ? 'pick' : 'none'; }
      }
    };
    await Promise.all([worker(), worker(), worker()]); // ค้นพร้อมกัน 3 เพลง
    imp.done = true;
  }
  const spCount = (st: SpItem['st']) => spImp?.items.filter((x) => x.st === st).length ?? 0;

  // ---------- สีเน้นแบบวนสี ----------
  let cyc = 0; // ตำแหน่งในรอบ (0..1) เก็บไว้นอก effect เปลี่ยนความเร็วแล้วสีไม่กระโดด
  $effect(() => {
    if (!cycling) return;
    const l = $state.snapshot(look) as Look;
    const stops = l.accentStops, sec = cycleSeconds(l.accentSpeed), r = document.documentElement;
    const put = () => { const v = accentVars(cycleColor(stops, cyc), l); r.style.setProperty('--accent', v['--accent']); r.style.setProperty('--on-accent', v['--on-accent']); };
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) { cyc = 0; put(); return; }
    let raf = 0, last = performance.now(), painted = 0;
    const tick = (now: number) => {
      cyc = (cyc + Math.min(now - last, 250) / 1000 / sec) % 1; // พับจอแล้วกลับมาไม่กระโดดไกล
      last = now;
      if (now - painted >= 50) { painted = now; put(); } // ~20 ครั้ง/วิ พอให้ลื่นและไม่กินเครื่อง
      raf = requestAnimationFrame(tick);
    };
    put();
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  });
  const setRgb = (i: number, v: number) => { const c = rgbOf(look.accent); c[i] = Math.max(0, Math.min(255, Math.round(v) || 0)); look.accent = toHex(...c); };
  const setHex = (v: string) => {
    let h = v.trim().toLowerCase().replace(/^#?/, '#');
    if (/^#[0-9a-f]{3}$/.test(h)) h = '#' + [...h.slice(1)].map((x) => x + x).join('');
    if (/^#[0-9a-f]{6}$/.test(h)) look.accent = h;
  };
  const saveAccent = () => (look.accentSaved = [look.accent, ...look.accentSaved.filter((x) => x !== look.accent)].slice(0, 24));
  const setStopCount = (n: number) => {
    const s = look.accentStops.slice(0, Math.max(2, Math.min(8, n)));
    while (s.length < n && s.length < 8) s.push(RAINBOW[s.length % RAINBOW.length]);
    look.accentStops = s;
  };
  const sameStops = (a: string[], b: string[]) => a.length === b.length && a.every((x, i) => x === b[i]);
  const saveSet = () => {
    const st = [...look.accentStops];
    if (look.accentSets.some((x) => sameStops(x.stops, st))) return say(tr('มีชุดสีนี้อยู่แล้ว'));
    look.accentSets = [{ name: tr('ชุดที่ {0}', look.accentSets.length + 1), stops: st }, ...look.accentSets].slice(0, 12);
  };
  const pickTheme = (t: (typeof THEMES)[number]) => (look = { ...look, accentMode: 'solid', accent: t.accent, base: t.base, raised: t.raised, outer: t.outer });
  const pickLayout = (l: (typeof LAYOUTS)[number]) => (look = { ...look, ...l.v });
  // รูปพื้นหลัง: คัดลอกเข้าโฟลเดอร์ของแอป (asset protocol อ่านได้เฉพาะที่นั่น) แล้วลบรูปเก่า
  let bgUrl = $state('');
  let bgBusy = $state(false);
  let bgMsg = $state('');
  // ใช้โทนสีจากรูปพื้นหลัง (GIF ใช้เฟรมแรก) สร้างสีเน้น/พื้นแผงให้เข้ากัน
  const isVideo = (p: string) => /\.(mp4|webm)$/i.test(p);
  // ตั้งค่าลดการเคลื่อนไหวของ Windows: หยุดวิดีโอพื้นหลังไว้ที่เฟรมแรก
  function noMotion(v: HTMLVideoElement) {
    const mq = matchMedia('(prefers-reduced-motion: reduce)');
    const f = () => (mq.matches ? v.pause() : v.play().catch(() => {}));
    f(); mq.addEventListener('change', f);
    return { destroy: () => mq.removeEventListener('change', f) };
  }
  async function pickBgVideo() {
    const f = await open({ filters: [{ name: tr('วิดีโอ'), extensions: ['mp4', 'webm'] }] });
    if (typeof f !== 'string') return;
    bgBusy = true; bgMsg = '';
    try { look.bg = await invoke<string>('import_bg_video', { src: f, old: look.bg || null }); } catch (e) { bgMsg = tr(String(e)); }
    bgBusy = false;
  }
  function matchBg() {
    if (!look.bg) return;
    bgMsg = '';
    if (isVideo(look.bg)) {
      const v = document.createElement('video');
      v.crossOrigin = 'anonymous'; v.muted = true; v.src = convertFileSrc(look.bg);
      v.onloadeddata = () => { v.currentTime = Math.min(1, v.duration / 2 || 0); };
      v.onseeked = () => {
        try {
          const c = document.createElement('canvas'); c.width = c.height = 48;
          const x = c.getContext('2d')!; x.drawImage(v, 0, 0, 48, 48);
          look = { ...look, ...paletteFromPixels(x.getImageData(0, 0, 48, 48).data) };
          bgMsg = tr('ปรับสีตามรูปแล้ว');
        } catch { bgMsg = tr('อ่านสีจากรูปนี้ไม่ได้'); }
      };
      v.onerror = () => (bgMsg = tr('อ่านสีจากรูปนี้ไม่ได้'));
      return;
    }
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      try {
        const c = document.createElement('canvas');
        c.width = c.height = 48;
        const x = c.getContext('2d')!;
        x.drawImage(img, 0, 0, 48, 48);
        look = { ...look, ...paletteFromPixels(x.getImageData(0, 0, 48, 48).data) };
        bgMsg = tr('ปรับสีตามรูปแล้ว');
      } catch {
        bgMsg = tr('อ่านสีจากรูปนี้ไม่ได้');
      }
    };
    img.onerror = () => (bgMsg = tr('อ่านสีจากรูปนี้ไม่ได้'));
    img.src = convertFileSrc(look.bg);
  }
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
      if (fading) applyVol(sleepLeft / 10); // ค่อยๆ เบาใน 10 วินาทีสุดท้าย
      if (sleepLeft <= 0) {
        audio.pause();
        fading = false;
        applyVol();
        sleepEnd = sleepMin = 0;
      }
    }, 500);
    return () => {
      clearInterval(timer);
      fading = false;
      if (audio) applyVol();
    };
  });

  // ---------- มินิเพลเยอร์ (หน้าต่างแยก คุยกันผ่าน event) ----------
  const npState = () => ({
    theme: themeVars,
    lang: i18n.lang,
    has: !!current, title: current?.title ?? '', artist: current?.artist ?? '', playing, pos, dur: current?.dur ?? 0, tint,
    cover: current?.cover ? convertFileSrc(current.cover) : null,
    line: ly.status === 'ready' && lyIdx >= 0 ? ly.lines[lyIdx]?.text || '♪' : ''
  });
  const pushNp = () => emit('np', npState()).catch(() => {});
  $effect(() => {
    pushNp();
  });
  async function openMini() {
    try {
      const ex = await WebviewWindow.getByLabel('mini');
      if (ex) return void (await ex.setFocus());
      new WebviewWindow('mini', { url: '/', title: 'MySound', width: 360, height: 184, resizable: false, alwaysOnTop: true });
    } catch (e) {
      notice = tr('เปิดมินิเพลเยอร์ไม่ได้: {0}', tr(String(e)));
    }
  }

  // ---------- เนื้อเพลง ----------
  type Ly = { status: 'loading' | 'none' | 'ready' | 'error'; lines: LrcLine[]; plain: string; msg?: string };
  const lyCache = new Map<number, Ly>();
  let ly = $state<Ly>({ status: 'none', lines: [], plain: '' });
  // แคชบนดิสก์ (lyrics-cache.json): s = แบบมีเวลา, p = ข้อความธรรมดา; null ทั้งคู่ = เคยค้นแล้วไม่เจอ
  type LyC = { s: string | null; p: string | null };
  let lyDisk = $state.raw<Record<number, LyC>>({});
  let lyDiskT = 0;
  onMount(() => {
    invoke<string>('lyrics_cache_read').then((j) => { if (j) lyDisk = { ...JSON.parse(j), ...lyDisk }; }).catch(() => {});
  });
  function cacheLy(id: number, c: LyC | null) {
    const n = { ...lyDisk };
    if (c) n[id] = c; else delete n[id];
    lyDisk = n;
    clearTimeout(lyDiskT);
    lyDiskT = window.setTimeout(() => invoke('lyrics_cache_write', { json: JSON.stringify(lyDisk) }).catch(() => {}), 1500);
  }
  function dropLy(id: number) { lyCache.delete(id); if (lyDisk[id]) cacheLy(id, null); }
  const lyFrom = (c: LyC): Ly => {
    const lines = c.s ? parseLrc(cleanLyrics(c.s)) : [];
    const plain = c.p ? cleanLyrics(c.p) : '';
    return lines.length || plain ? { status: 'ready', lines, plain } : { status: 'none', lines: [], plain: '' };
  };
  // กำลังดึงอยู่ (ตอนพรีโหลดเพลงถัดไป) ถ้าผู้ใช้เปลี่ยนมาเพลงนั้นพอดีจะรอผลเดียวกัน ไม่ยิงซ้ำ
  const lyFlight = new Map<number, Promise<LyC>>();
  function fetchLy(t: Track): Promise<LyC> {
    let p = lyFlight.get(t.id);
    if (!p) {
      p = invoke<{ synced: string | null; plain: string | null }>('lyrics', { title: t.title, artist: t.artist, dur: t.dur })
        .then((d) => {
          const c = { s: d.synced, p: d.plain };
          lyCache.set(t.id, lyFrom(c));
          cacheLy(t.id, c);
          return c;
        })
        .finally(() => lyFlight.delete(t.id));
      lyFlight.set(t.id, p);
    }
    return p;
  }
  async function loadLyrics(t: Track, force = false) {
    if (t.lrc) {
      const src = cleanLyrics(t.lrc);
      const lines = parseLrc(src);
      return void (ly = { status: 'ready', lines, plain: lines.length ? '' : src });
    }
    const hit = lyCache.get(t.id);
    if (hit && !force) return void (ly = hit);
    const dc = lyDisk[t.id];
    if (dc && !force) {
      const r0 = lyFrom(dc);
      lyCache.set(t.id, r0);
      return void (ly = r0);
    }
    ly = { status: 'loading', lines: [], plain: '' };
    let r: Ly;
    try {
      r = lyFrom(await fetchLy(t));
    } catch (e) {
      r = { status: 'error', lines: [], plain: '', msg: tr(String(e)) };
    }
    if (current?.id === t.id) ly = r;
  }
  $effect(() => {
    if ((panel === 'lyrics' || barLy || karaoke || lyFloatOpen || (petOn && petLy) || (discord && discordLy)) && current) loadLyrics(current);
  });
  // พรีโหลดเนื้อเพลงของ 2 เพลงถัดไปในคิวขณะเพลงนี้เล่น พอถึงเพลงนั้นก็ขึ้นทันที
  $effect(() => {
    if (!(panel === 'lyrics' || barLy || karaoke || lyFloatOpen || (petOn && petLy) || (discord && discordLy)) || !current || !playing) return;
    const ahead = queue.slice(0, 2).filter((t) => !t.lrc && !lyCache.has(t.id) && !lyDisk[t.id] && !lyFlight.has(t.id));
    const timer = setTimeout(() => ahead.forEach((t) => void fetchLy(t).catch(() => {})), 1500); // ให้เพลงปัจจุบันได้ก่อน
    return () => clearTimeout(timer);
  });
  const lyOff = $derived(current?.lyOff ?? 0); // วินาที: บวก = เนื้อเพลงขึ้นช้าลง
  const lyIdx = $derived(ly.lines.length ? lrcIndex(ly.lines, pos - lyOff) : -1);
  function nudgeLy(d: number) {
    if (!current) return;
    current.lyOff = Math.round(((current.lyOff ?? 0) + d) * 10) / 10 || 0;
  }
  const barLine = $derived(barLy && current && ly.status === 'ready' && lyIdx >= 0 ? ly.lines[lyIdx].text || '♪' : '');
  // ---------- เนื้อเพลงใน Discord: เลือกบรรทัดล่วงหน้า 2 วินาที (กันหน่วง) ส่งแบบโควตา ไม่เกิน 4 ครั้งต่อ 20 วินาที ----------
  const dIdx = $derived(discord && discordLy && playing && ly.status === 'ready' && ly.lines.length ? lrcIndex(ly.lines, pos + 2 - lyOff) : -1); // เลือกล่วงหน้า 2 วินาที ชดเชยการส่ง+การวาดของ Discord
  // ตัวจำกัดความถี่แบบโควตา: Discord รับอัปเดตสถานะประมาณ 5 ครั้ง/20 วินาที เราใช้ไม่เกิน 4 ครั้งต่อ 20 วินาที
  // (เหลือไว้ให้ pause/เปลี่ยนเพลง) และห่างกันอย่างน้อย 1.2 วินาที บรรทัดที่ห่างกันนานจึงส่งทันทีไม่ต้องรอ 5 วินาทีทุกครั้ง
  const D_WINDOW = 20000, D_MAX = 4, D_GAP = 1200;
  let dSends: number[] = [];
  function dWait(now: number) {
    dSends = dSends.filter((t) => now - t < D_WINDOW);
    const byWindow = dSends.length >= D_MAX ? dSends[dSends.length - D_MAX] + D_WINDOW - now : 0;
    const byGap = dSends.length ? dSends[dSends.length - 1] + D_GAP - now : 0;
    return Math.max(0, byWindow, byGap);
  }
  $effect(() => {
    void current?.id; // เปลี่ยนเพลง: ล้างบรรทัดเก่าทันที
    dLine = '';
    dSends = [];
  });
  $effect(() => {
    const want = dIdx >= 0 ? ly.lines[dIdx].text.trim() : '';
    if (want === untrack(() => dLine)) return;
    const timer = setTimeout(() => {
      dLine = want;
      dSends.push(Date.now());
    }, dWait(Date.now()));
    return () => clearTimeout(timer);
  });

  $effect(() => {
    if (lyIdx < 0 || panel !== 'lyrics') return;
    const smooth = !matchMedia('(prefers-reduced-motion: reduce)').matches;
    tick().then(() => document.querySelector('.ly .on')?.scrollIntoView({ block: 'center', behavior: smooth ? 'smooth' : 'auto' }));
  });

  let lyDlg = $state<HTMLDialogElement>();
  let lyText = $state('');
  let lyMsg = $state('');
  // ---------- โหลดเนื้อเพลงทั้งคลังไว้ล่วงหน้า ----------
  let pre = $state<{ done: number; total: number; found: number } | null>(null);
  let preStop = false;
  async function prefetchLyrics() {
    const todo = library.filter((t) => !t.lrc && !lyDisk[t.id]);
    pre = { done: 0, total: todo.length, found: 0 };
    preStop = false;
    for (const t of todo) {
      if (preStop) break;
      try {
        const d = await invoke<{ synced: string | null; plain: string | null }>('lyrics', { title: t.title, artist: t.artist, dur: t.dur });
        cacheLy(t.id, { s: d.synced, p: d.plain });
        if (d.synced || d.plain) pre.found++;
      } catch { /* เน็ตหลุด/เซิร์ฟเวอร์ล่ม: ข้ามเพลงนี้ */ }
      pre.done++;
      await new Promise((r) => setTimeout(r, 250)); // ไม่ยิง LRCLIB ถี่เกินไป
    }
    say(tr('โหลดเนื้อเพลงแล้ว เจอ {0} จาก {1} เพลง', pre.found, pre.done));
    pre = null;
  }
  const lyCount = $derived(Object.values(lyDisk).filter((c) => c.s || c.p).length);
  // ---------- ค้นหาจากเนื้อเพลง ----------
  const stripTags = (x: string) => x.replace(/\[[^\]]*\]/g, '').trim();
  const lyTextOf = (t: Track) => t.lrc ?? lyDisk[t.id]?.s ?? lyDisk[t.id]?.p ?? '';
  const lyHits = $derived.by(() => {
    const w = norm(pq.words.join(' '));
    if (view !== 'songs' || w.length < 3) return [];
    const inShown = new Set(shown.map((t) => t.id));
    const out: { t: Track; line: string; at: number | null }[] = [];
    for (const t of library) {
      if (inShown.has(t.id)) continue;
      const raw = lyTextOf(t).split(/\r?\n/).find((l) => norm(stripTags(l)).includes(w));
      if (raw) {
        const m = /\[(\d+):(\d+(?:\.\d+)?)\]/.exec(raw);
        out.push({ t, line: stripTags(raw), at: m ? +m[1] * 60 + +m[2] : null });
      }
      if (out.length >= 30) break;
    }
    return out;
  });
  function playAtLine(h: { t: Track; at: number | null }) {
    play(h.t);
    if (h.at != null) pendingSeek = Math.max(0, h.at - 1);
  }
  // ---------- ส่งออก .lrc ----------
  let lrcMsg = $state('');
  async function exportLrc() {
    const items = library.flatMap((t) => {
      const txt = t.lrc && /\[\d+:\d+/.test(t.lrc) ? t.lrc : lyDisk[t.id]?.s;
      return txt ? [[`${tr(t.artist)} - ${t.title}`, `[ti:${t.title}]\n[ar:${tr(t.artist)}]\n${t.album ? `[al:${t.album}]\n` : ''}${txt}`] as [string, string]] : [];
    });
    if (!items.length) return void (lrcMsg = tr('ยังไม่มีเพลงที่มีเนื้อเพลงแบบมีเวลา ลองกด “โหลดเนื้อเพลงทั้งคลัง” ก่อน'));
    const dir = await open({ directory: true, title: tr('เลือกโฟลเดอร์ปลายทาง') });
    if (typeof dir !== 'string') return;
    try {
      const n = await invoke<number>('write_lrc_files', { dir, items });
      lrcMsg = tr('ส่งออกแล้ว {0} ไฟล์', n);
    } catch (e) {
      lrcMsg = tr(String(e));
    }
  }
  // ---------- คาราโอเกะเต็มจอ ----------
  let karaoke = $state(false);
  let kpos = $state(0);
  $effect(() => {
    if (!karaoke) return;
    let raf = 0;
    const tick = () => { kpos = audio?.currentTime ?? 0; raf = requestAnimationFrame(tick); };
    tick();
    return () => cancelAnimationFrame(raf);
  });
  const kIdx = $derived(karaoke && ly.lines.length ? lrcIndex(ly.lines, kpos - lyOff) : -1);
  const kProg = $derived.by(() => {
    if (kIdx < 0) return 0;
    const t0 = ly.lines[kIdx].t + lyOff, t1 = (ly.lines[kIdx + 1]?.t ?? ly.lines[kIdx].t + 4) + lyOff;
    return Math.max(0, Math.min(1, (kpos - t0) / Math.max(0.3, t1 - t0)));
  });
  function openKaraoke() {
    karaoke = true;
    document.documentElement.requestFullscreen?.().catch(() => {});
  }
  function closeKaraoke() {
    karaoke = false;
    if (document.fullscreenElement) document.exitFullscreen().catch(() => {});
  }
  onMount(() => {
    const f = () => { if (!document.fullscreenElement && karaoke) karaoke = false; };
    document.addEventListener('fullscreenchange', f);
    return () => document.removeEventListener('fullscreenchange', f);
  });
  // ---------- หน้าต่างเนื้อเพลงลอย ----------
  let lyFloatOpen = $state(false);
  const curLine = $derived(lyIdx >= 0 ? ly.lines[lyIdx]?.text || '♪' : '');
  const nextLine = $derived(lyIdx + 1 < ly.lines.length && ly.lines.length ? ly.lines[lyIdx + 1].text : '');
  const pushLyFloat = () =>
    emit('lyfloat', { cur: ly.status === 'ready' ? curLine : '', next: ly.status === 'ready' ? nextLine : '', title: current ? `${current.title} · ${tr(current.artist)}` : '', accent: themeVars['--accent'], lang: i18n.lang }).catch(() => {});
  $effect(() => {
    void [curLine, nextLine, current?.id, ly.status, themeVars];
    if (lyFloatOpen) pushLyFloat();
  });
  onMount(() => {
    const un = listen('lyfloat-ready', () => pushLyFloat());
    return () => void un.then((f) => f());
  });
  async function openLyFloat() {
    try {
      const ex = await WebviewWindow.getByLabel('lyrics');
      if (ex) return void (await ex.setFocus());
      const w = new WebviewWindow('lyrics', { url: '/', title: 'MySound Lyrics', width: 680, height: 130, transparent: true, decorations: false, alwaysOnTop: true, skipTaskbar: true, shadow: false });
      lyFloatOpen = true;
      w.once('tauri://destroyed', () => (lyFloatOpen = false));
    } catch (e) {
      notice = tr(String(e));
    }
  }
  // ---------- ตั้งเวลาเนื้อเพลงเอง (แตะตามจังหวะ) ----------
  let sync = $state<{ text: string; t: number | null }[] | null>(null);
  let syncI = $state(0);
  const lrcTime = (t: number) => `${String(Math.floor(t / 60)).padStart(2, '0')}:${(t % 60).toFixed(2).padStart(5, '0')}`;
  // ---------- AI จับเวลาเนื้อเพลง (whisper.cpp รุ่น small รันในเครื่อง) ----------
  type WPart = { phase: 'missing' | 'downloading' | 'paused' | 'verifying' | 'extracting' | 'ready' | 'failed'; done: number; total: number; error: string | null };
  type WState = { engine: WPart; model: WPart; running: boolean };
  let ai = $state<{ stage: 'engine' | 'model' | 'extract' | 'run'; pct: number } | null>(null);
  let aiWait: ((s: WState) => void) | null = null;
  function aiFromState(st: WState) {
    const e = st.engine, m = st.model;
    const pct = (p: WPart) => (p.total ? Math.round((p.done / p.total) * 100) : 0);
    if (e.phase === 'extracting' || e.phase === 'verifying') ai = { stage: 'extract', pct: 0 };
    else if (e.phase === 'downloading') ai = { stage: 'engine', pct: pct(e) };
    else if (m.phase === 'downloading' || m.phase === 'verifying') ai = { stage: 'model', pct: pct(m) };
    if (!st.running) { const f = aiWait; aiWait = null; f?.(st); }
  }
  onMount(() => {
    const u1 = listen<WState>('whisper-state', (e) => { if (ai) aiFromState(e.payload); });
    const u2 = listen<number>('whisper-progress', (e) => { if (ai) ai = { stage: 'run', pct: e.payload }; });
    return () => { void u1.then((f) => f()); void u2.then((f) => f()); };
  });
  /** เตรียม AI ให้พร้อม (โหลดต่อจากที่ค้างไว้ได้) คืนค่าเมื่อจบ ถ้าไม่ครบจะโยนเหตุผลออกมา */
  async function aiPrepare() {
    const done = new Promise<WState>((res) => (aiWait = res));
    const now = await invoke<WState>('whisper_setup');
    aiFromState(now);
    const st = await done;
    if (st.engine.phase === 'ready' && st.model.phase === 'ready') return;
    const bad = st.engine.phase !== 'ready' ? st.engine : st.model;
    if (!bad.error) throw new Error(tr('ยกเลิกการโหลดแล้ว กดอีกครั้งเพื่อโหลดต่อจากเดิม'));
    throw new Error(tr('โหลด AI ไม่สำเร็จ: {0} (กดอีกครั้งเพื่อโหลดต่อจากเดิม)', tr(bad.error)));
  }
  const aiCancel = () => void invoke(ai?.stage === 'run' ? 'whisper_cancel' : 'whisper_setup_cancel');
  async function aiSync() {
    const t = current;
    const lines = lyText.split(/\r?\n/).map(stripTags).filter(Boolean);
    if (!lines.length) return void (lyMsg = tr('วางเนื้อเพลงแบบข้อความธรรมดาลงช่องก่อน แล้วค่อยเริ่มตั้งเวลา'));
    if (!t || ai) return;
    lyMsg = '';
    try {
      const st = await invoke<WState>('whisper_state');
      if (st.engine.phase !== 'ready' || st.model.phase !== 'ready') {
        const resume = st.engine.phase === 'paused' || st.model.phase === 'paused';
        const ok = await new Promise<boolean>((res) => {
          ask(resume ? tr('โหลด AI ค้างอยู่ จะโหลดต่อจากเดิมเลยไหม?') : tr('ครั้งแรกต้องโหลดตัว AI (whisper รุ่น small) ประมาณ 480 MB เก็บไว้ในเครื่อง ครั้งต่อไปไม่ต้องโหลดอีก โหลดเลยไหม?'), resume ? tr('โหลดต่อ') : tr('โหลด'), () => res(true));
          confirmDlg?.addEventListener('close', () => setTimeout(() => res(false), 0), { once: true });
        });
        if (!ok) return;
        ai = { stage: st.engine.phase === 'ready' ? 'model' : 'engine', pct: 0 };
        await aiPrepare();
      }
      ai = { stage: 'run', pct: 0 };
      // ไม่ส่งเนื้อเพลงเป็น prompt แล้ว: whisper จะ "เดา" คำในเนื้อเพลงออกมาทั้งที่ยังไม่มีคนร้อง (ช่วงอินโทร) ทำให้บรรทัดขึ้นก่อนเวลา/ฟังวน
      const words = await invoke<Word[]>('whisper_run', { path: t.path, lang: /[\u0e00-\u0e7f]/.test(lines.join('')) ? 'th' : 'auto' });
      const { times, conf } = alignDetail(lines, words);
      const found = times.filter((x) => x !== null).length;
      if (!found) throw new Error(tr('AI ไม่ได้ยินเสียงร้องที่ตรงกับเนื้อเพลงเลย ลองตั้งเวลาเองแทน'));
      const full = fillGaps(times, t.dur);
      lyText = lines.map((l, i) => `[${lrcTime(Math.max(0, full[i] - 0.1))}]${l}`).join('\n');
      const weak = weakLines(times, conf, full);
      lyMsg = weak.length
        ? tr('AI จับเวลาได้ {0} จาก {1} บรรทัด · ควรตรวจบรรทัดที่ {2} (ไม่แน่ใจ) แล้วกดบันทึก', found, lines.length, weak.slice(0, 12).map((i) => i + 1).join(', ') + (weak.length > 12 ? '…' : ''))
        : tr('AI จับเวลาได้ครบทุกบรรทัด ตรวจแล้วกดบันทึก');
    } catch (e) {
      lyMsg = tr(String(e instanceof Error ? e.message : e));
    } finally {
      ai = null;
    }
  }
  const aiText = $derived(ai ? (ai.stage === 'extract' ? tr('กำลังติดตั้งตัวโปรแกรม AI…') : ai.stage === 'engine' ? tr('กำลังโหลดตัวโปรแกรม AI… {0}%', ai.pct) : ai.stage === 'model' ? tr('กำลังโหลดโมเดล AI… {0}%', ai.pct) : tr('AI กำลังฟังเพลง… {0}%', ai.pct)) : '');
  function startSync() {
    const lines = lyText.split(/\r?\n/).map(stripTags).filter(Boolean);
    if (!lines.length) return void (lyMsg = tr('วางเนื้อเพลงแบบข้อความธรรมดาลงช่องก่อน แล้วค่อยเริ่มตั้งเวลา'));
    if (!audio || !current) return;
    sync = lines.map((text) => ({ text, t: null }));
    syncI = 0;
    lyMsg = '';
    audio.currentTime = 0;
    audio.play().catch(() => {});
    queueMicrotask(() => document.querySelector<HTMLButtonElement>('.syncbtn')?.focus());
  }
  function stamp() {
    if (!sync || !audio || syncI >= sync.length) return;
    sync[syncI].t = Math.max(0, audio.currentTime - 0.15); // ชดเชยเวลากดช้ากว่าเสียงเล็กน้อย
    syncI++;
    if (syncI >= sync.length) finishSync();
  }
  function unstamp() {
    if (!sync || !audio || syncI === 0) return;
    syncI--;
    sync[syncI].t = null;
    audio.currentTime = Math.max(0, (sync[syncI - 1]?.t ?? 0) - 0.5);
  }
  function finishSync() {
    if (!sync) return;
    lyText = sync.filter((l) => l.t !== null).map((l) => `[${lrcTime(l.t!)}]${l.text}`).join('\n');
    sync = null;
    lyMsg = tr('ตั้งเวลาเสร็จแล้ว ตรวจแล้วกดบันทึก');
  }
  function syncKey(e: KeyboardEvent) {
    if (!sync) return;
    if (e.code === 'Space' || e.key === 'Enter') { e.preventDefault(); e.stopPropagation(); stamp(); }
    else if (e.key === 'Backspace') { e.preventDefault(); e.stopPropagation(); unstamp(); }
  }
  $effect(() => {
    void syncI;
    if (sync) queueMicrotask(() => document.querySelector('.syncl.on')?.scrollIntoView({ block: 'center' }));
  });
  function openLyEdit() {
    if (!current) return;
    lyText = current.lrc ?? (ly.status === 'ready' ? (ly.lines.length ? ly.lines.map((l) => l.text).join('\n') : ly.plain) : '');
    sync = null;
    lyMsg = '';
    openDlg(lyDlg);
  }
  async function pickLrc() {
    const f = await open({ filters: [{ name: tr('เนื้อเพลง'), extensions: ['lrc', 'txt'] }] });
    if (typeof f !== 'string') return;
    try {
      lyText = cleanLyrics(await invoke<string>('read_text_file', { path: f }));
      lyMsg = '';
    } catch (e) {
      lyMsg = tr(String(e));
    }
  }
  function saveLy(clear = false) {
    if (!current) return;
    const t = clear ? '' : cleanLyrics(lyText).trim(); // ลบ [Intro] [Chorus] ฯลฯ ก่อนเก็บ
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
    if (!p) return;
    if (p.ids.includes(tid)) return say(tr('เพลงนี้อยู่ใน “{0}” แล้ว', p.name));
    p.ids.push(tid);
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
  const toggleFav = (t: Track) => { t.fav = !t.fav; if (t.fav && petWinId && t.id === current?.id) emit('pet-heart').catch(() => {}); };

  // ---------- ลบ (มีหน้าต่างยืนยันเสมอ) ----------
  let confirmDlg = $state<HTMLDialogElement>();
  let confirmS = $state<{ msg: string; label: string; ok: () => void }>({ msg: '', label: '', ok: () => {} });
  function ask(msg: string, label: string, ok: () => void) {
    confirmS = { msg, label, ok };
    openDlg(confirmDlg);
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
  const askDeleteTrack = (t: Track) => toTrash([t]);
  // ---------- คลิกขวาที่เพลย์ลิสต์: เปลี่ยนชื่อ / รูป / ปักหมุด / ลบ ----------
  let plCtx = $state<{ x: number; y: number; p: Playlist } | null>(null);
  function plMenu(e: MouseEvent, p: Playlist) {
    e.preventDefault();
    ctx = null;
    plCtx = { x: e.clientX, y: e.clientY, p };
  }
  function plRename(p: Playlist) {
    plCtx = null;
    view = `pl:${p.id}`;
    tick().then(startRename);
  }
  async function plPickImage(p: Playlist) {
    plCtx = null;
    const f = await open({ filters: [{ name: tr('รูปภาพ'), extensions: ['jpg', 'jpeg', 'png', 'webp', 'gif'] }] });
    if (typeof f !== 'string') return;
    try {
      const r = await invoke<{ path: string }>('import_cover', { key: `pl-${p.id}`, source: f, old: p.image ?? null });
      p.image = r.path;
    } catch (e) { notice = tr(String(e)); }
  }
  function plRemoveImage(p: Playlist) {
    plCtx = null;
    if (p.image) invoke('delete_files', { paths: [p.image] }).catch(() => {});
    p.image = undefined;
  }
  function plDelete(p: Playlist) {
    plCtx = null;
    ask(tr('ลบเพลย์ลิสต์ “{0}”? (เพลงในคลังยังอยู่ครบ)', p.name), tr('ลบเพลย์ลิสต์'), () => {
      checkpoint();
      playlists = playlists.filter((x) => x.id !== p.id);
      if (view === `pl:${p.id}`) view = 'home';
    });
  }
  // ลบเพลย์ลิสต์พร้อมเพลงในนั้น: เพลงไปถังขยะ (กู้คืนได้) ย้อนกลับครั้งเดียวได้ทั้งเพลย์ลิสต์และเพลง
  function plDeleteAll(p: Playlist) {
    plCtx = null;
    const find = () => p.ids.map((id) => library.find((t) => t.id === id)).filter((t): t is Track => !!t);
    const n = find().length;
    ask(tr('ลบเพลย์ลิสต์ “{0}” พร้อมเพลง {1} เพลงในนั้น? เพลงจะไปอยู่ในถังขยะ (กู้คืนได้) และหายจากเพลย์ลิสต์อื่นด้วย', p.name, n), tr('ลบทั้งหมด'), () => {
      const ts = find();
      if (ts.length) toTrash(ts); else checkpoint();
      playlists = playlists.filter((x) => x.id !== p.id);
      if (view === `pl:${p.id}`) view = 'home';
      say(tr('ลบเพลย์ลิสต์ “{0}” และ {1} เพลงแล้ว', p.name, ts.length), { label: tr('ย้อนกลับ'), fn: undo });
    });
  }
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
    plCtx = null;
    const h = 40 + 34 * (8 + playlists.length + (plId ? 3 : 0));
    ctx = { x: Math.min(e.clientX, innerWidth - 230), y: Math.min(e.clientY, innerHeight - h), t, plId };
  }
  const act = (f: () => void) => () => (f(), (ctx = null));

  // ---------- ตั้งค่าเพลง ----------
  let dlg = $state<HTMLDialogElement>();
  let edit = $state({ id: 0, title: '', artist: '', album: '', src: '', tags: '', note: '' });
  let editMsg = $state('');
  let editOk = $state(false);
  let busy = $state(false);
  const editing = $derived(library.find((t) => t.id === edit.id));
  function openEdit(t: Track) {
    ctx = null;
    clipA = 0; clipB = Math.round(t.dur); cutMsg = '';
    edit = { id: t.id, title: t.title, artist: t.artist, album: t.album, src: '', tags: (t.tags ?? []).join(', '), note: t.note ?? '' };
    editMsg = '';
    openDlg(dlg);
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
      checkpoint();
      editing.tags = splitTags(edit.tags);
      editing.note = edit.note.trim() || undefined;
      editing.title = edit.title.trim();
      editing.artist = edit.artist.trim() || editing.artist;
      editing.album = edit.album.trim() || editing.album;
      dropLy(editing.id);
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
    openDlg(palDlg);
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
    cur?: string; idx?: number; n?: number; skipped?: number;
    why?: { m: string; n: number }[]; // สาเหตุที่ข้าม (เพลย์ลิสต์)
    meta?: { title: string; artist: string; album: string }; // ชื่อ/ศิลปินจาก Spotify (สะอาดกว่าชื่อคลิป)
  };
  let downloads = $state<Dl[]>([]);
  let url = $state('');
  let err = $state('');
  let urlInput = $state<HTMLInputElement>();
  let seq = 0;

  function onTrackDone(job: number, r: Done) {
    const d = downloads.find((x) => x.id === job);
    if (d) d.count++; // นับทุกเพลงที่ yt-dlp รายงาน แม้อยู่ในคลังอยู่แล้ว
    // เพลงที่มีไฟล์อยู่ในคลังแล้ว (เคยโหลดมาก่อน) ไม่สร้างซ้ำ แต่ยังต้องใส่เข้าเพลย์ลิสต์นี้ด้วย
    let id = library.find((t) => t.path === r.path)?.id;
    if (id === undefined) {
      const today = new Date().toISOString().slice(0, 10);
      id = Date.now() + seq++;
      const sp = splitArtistTitle(r.title); // "ศิลปิน - ชื่อเพลง" → แยกใส่ช่องศิลปิน
      const m = d?.meta;
      library.unshift({ id, src: d?.src ?? 'youtube', added: today, last: today, plays: 0, title: m?.title ?? sp?.title ?? r.title, artist: m?.artist ?? sp?.artist ?? r.artist, album: m?.album || r.album, dur: r.dur, path: r.path, cover: r.cover, coverUrl: r.thumb, url: r.url });
      if (d && !d.playlist && !m) d.title = r.title;
    }
    const put = (pid: number) => { const p = playlists.find((x) => x.id === pid); if (p && !p.ids.includes(id)) p.ids.push(id); }; // เงียบ ไม่เด้งแจ้งซ้ำตอนโหลดทั้งลิสต์
    if (r.playlist) put(playlists.find((p) => p.name === r.playlist)?.id ?? newPlaylist(r.playlist));
    const w = r.url ? plWait.get(linkKey(r.url)) : undefined;
    if (w !== undefined) { put(w); plWait.delete(linkKey(r.url)); }
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
  // คิวดาวน์โหลด: โหลดพร้อมกันได้ 2 รายการ ที่เหลือรอคิว
  const startedDl = new Set<number>();
  let dlRunning = 0;
  function pump() {
    for (const d of [...downloads].reverse()) {
      if (dlRunning >= 2) break;
      if (d.state !== 'waiting' || startedDl.has(d.id)) continue;
      startedDl.add(d.id);
      dlRunning++;
      runJob(d).finally(() => { dlRunning--; pump(); });
    }
  }
  const running = new Set<number>(); // กันงานเดียวกันถูกสั่งซ้อน (กดลองใหม่รัวๆ / คิว + ปุ่ม)
  async function runJob(d: Dl) {
    if (running.has(d.id)) return;
    running.add(d.id);
    d.state = 'waiting';
    d.msg = undefined;
    d.pct = 0; d.count = 0; d.idx = 0; d.n = 0; d.skipped = 0; d.cur = undefined; d.why = undefined;
    try {
      await invoke('download', { id: d.id, url: d.url, playlist: d.playlist, cookies: ytCookies });
      d.state = 'done';
      d.pct = 100;
    } catch (e) {
      d.state = 'failed';
      d.msg = tr(String(e));
    } finally {
      running.delete(d.id);
      startedDl.delete(d.id);
    }
  }
  async function addDownload(raw?: string) {
    const u = (raw ?? url).trim();
    if (/^spotify:|open\.spotify\.com\//.test(u)) {
      if (!spUser) { err = tr('ลิงก์ Spotify ต้องเชื่อม Spotify ก่อน: เครื่องมือ → เชื่อมต่อ (ต้องใช้ Spotify Premium)'); return; }
      if (/\/track\//.test(u)) { err = tr('ตอนนี้รองรับลิงก์เพลย์ลิสต์และอัลบั้มของ Spotify (ยังไม่รองรับลิงก์เพลงเดี่ยว)'); return; }
      url = '';
      return spImport(u);
    }
    const r = prepare(u);
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

<svelte:window onpointermove={onPointerMove} onpointerup={onPointerUp} onkeydown={onKey} onkeyup={onKeyUp} onpaste={onPaste} onclick={() => ((ctx = null), (plCtx = null), (npMore = false))} oncontextmenu={(e) => { if (e.defaultPrevented) return; ctx = null; if (!(e.target as HTMLElement)?.closest?.('input,textarea,[contenteditable]')) e.preventDefault(); }} />

{#each [0, 1] as i (i)}
  <audio
    bind:this={decks[i]}
    onplay={() => decks[i] === audio && (playing = true)}
    onpause={() => decks[i] === audio && !decks[i].ended && (playing = false)}
    ontimeupdate={() => decks[i] === audio && onTime()}
    onloadedmetadata={() => decks[i] === audio && onMeta()}
    onseeked={() => decks[i] === audio && seeks++}
    onended={() => decks[i] === audio && ended()}
    onerror={() => decks[i] === audio && audioError()}
    onplaying={() => { if (decks[i] === audio) { corsTried = null; attachViz(); } }}
    crossorigin="anonymous"
  ></audio>
{/each}

{#snippet card(t: Track, list: Track[])}
  <button class="card" onclick={() => play(t, list)} oncontextmenu={(e) => menu(e, t)}>
    <div class="art" style:background={art(t)}>
      <span class="fab"><Icon n="play" size={18} /></span>
    </div>
    <b>{t.title}</b>
    <small>{tr(t.artist)}</small>
  </button>
{/snippet}

{#snippet sh(k: string, label: string, cls = '')}
  <span role="columnheader" class="sortc {cls}" aria-sort={sortKey === k ? (sortDir === 1 ? 'ascending' : 'descending') : 'none'}><button class="sorth" class:on={sortKey === k} onclick={() => sortBy(k)}>{label}{sortKey === k ? (sortDir === 1 ? ' ↑' : ' ↓') : ''}</button></span>
{/snippet}

{#snippet rows(list0: Track[], plId?: number)}
  {@const list = sorted(list0)}
  {@const _v = (vis.list = list)}
  <div class="rows" style:--cols={colsTpl}>
  <div class="listtools">
    <details class="colmenu">
      <summary>{tr('คอลัมน์')}</summary>
      <div class="colpop">
        {#each [['album', 'อัลบั้ม'], ['added', 'วันที่เพิ่ม'], ['plays', 'จำนวนครั้งที่เล่น'], ['rating', 'ดาว'], ['bpm', 'BPM'], ['dur', 'เวลา']] as [k, l] (k)}
          <label><input type="checkbox" name={'col-' + k} bind:checked={cols[k as keyof typeof cols]} /> {tr(l)}</label>
        {/each}
      </div>
    </details>
    {#if sortKey}<button class="link" onclick={() => (sortKey = '')}>{tr('เรียงตามเดิม')}</button>{/if}
    <div class="seg" role="group" aria-label={tr('มุมมอง')}>
      <button aria-pressed={look.listMode === 'rows'} class:on={look.listMode === 'rows'} onclick={() => (look.listMode = 'rows')} aria-label={tr('แบบแถว')} title={tr('แบบแถว')}><Icon n="queue" size={16} /></button>
      <button aria-pressed={look.listMode === 'grid'} class:on={look.listMode === 'grid'} onclick={() => (look.listMode = 'grid')} aria-label={tr('แบบการ์ด')} title={tr('แบบการ์ด')}><Icon n="grid" size={16} /></button>
    </div>
  </div>
  {#if look.listMode !== 'grid'}<div class="thead" role="row">
    <span>#</span>{@render sh('title', tr('ชื่อเพลง'))}
    {#if cols.album}{@render sh('album', tr('อัลบั้ม'))}{/if}
    {#if cols.added}{@render sh('added', tr('วันที่เพิ่ม'))}{/if}
    {#if cols.plays}{@render sh('plays', tr('เล่น'), 'r')}{/if}
    {#if cols.rating}{@render sh('rating', tr('ดาว'))}{/if}
    {#if cols.bpm}{@render sh('bpm', 'BPM', 'r')}{/if}
    <span></span>
    {#if cols.dur}{@render sh('dur', tr('เวลา'), 'r')}{/if}
    <span></span>
  </div>{/if}
  {#if look.listMode === 'grid'}
    <div class="cards gridlist">{#each list as t (t.id)}{@render card(t, list)}{:else}<p class="empty">{library.length ? tr('ไม่พบเพลง') : tr('ยังไม่มีเพลง')}</p>{/each}</div>
  {:else}
  {@const [wa, wb] = winRange(list.length)}
  <div use:measureTop style:padding-top="{wa * look.row}px" style:padding-bottom="{(list.length - wb) * look.row}px">
  {#each list.slice(wa, wb) as t, j (t.id)}
    {@const i = wa + j}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      in:rowHeight={{ n: list.length }}
      out:rowHeight={{ n: list.length }}
      animate:rowFlip
      data-idx={i}
      class:stag={stagger && i < 12}
      style:--d={i}
      class:lifted={!!drag && drag.ids.includes(t.id)}
      class:shiftdn={drag?.over != null && i >= drag.over}
      class:shiftup={drag?.over != null && i < drag.over}
      class:dropbefore={drag?.over === i}
      class:dropafter={drag?.over === i + 1 && i === list.length - 1}
      onpointerdown={(e) => rowDown(e, t, list, plId)}
      class="row"
      role="row"
      tabindex="0"
      aria-selected={selIds.includes(t.id)}
      class:sel={selIds.includes(t.id)}
      class:now={current?.id === t.id}
      class:gone={missing.includes(t.id)}
      onclick={(e) => !justDragged && rowClick(e, t, i, list)}
      oncontextmenu={(e) => menu(e, t, plId)}
      onkeydown={(e) => e.target === e.currentTarget && e.key === 'Enter' && play(t, list)}
    >
      <span class="num"><i>{current?.id === t.id && playing ? '♪' : i + 1}</i><em><Icon n="play" size={14} /></em></span>
      <span class="title">
        <span class="thumb" style:background={art(t)}></span>
        <span><b>{t.title}</b><small>{missing.includes(t.id) ? tr('ไฟล์หาย · ') : ''}<button class="alink" onclick={(e) => (e.stopPropagation(), (view = 'artist:' + t.artist))}>{tr(t.artist)}</button>{#if t.rating && !cols.rating} · {'★'.repeat(t.rating)}{/if}{#if t.tags?.length} · <span class="tags">{t.tags.join(', ')}</span>{/if}</small></span>
      </span>
      {#if cols.album}<span class="dim">{#if t.album}<button class="alink" onclick={(e) => (e.stopPropagation(), (view = 'album:' + t.album))}>{t.album}</button>{/if}</span>{/if}
      {#if cols.added}<span class="dim">{fmtDate(t.added, i18n.lang)}</span>{/if}
      {#if cols.plays}<span class="dim r">{t.plays}</span>{/if}
      {#if cols.rating}<span class="dim stars-s">{'★'.repeat(t.rating ?? 0)}</span>{/if}
      {#if cols.bpm}<span class="dim r">{t.bpm ?? '–'}</span>{/if}
      <button class="heart" class:on={t.fav} aria-label={t.fav ? tr('เลิกถูกใจ {0}', t.title) : tr('ถูกใจ {0}', t.title)} aria-pressed={!!t.fav} onclick={(e) => (e.stopPropagation(), favClick(e, t))} ondblclick={(e) => e.stopPropagation()}><Icon n="heart" size={16} fill={!!t.fav} /></button>
      {#if cols.dur}<span class="dim r">{fmt(t.dur)}</span>{/if}
      <button class="more" aria-label={tr('ตัวเลือกเพิ่มเติมของ {0}', t.title)} aria-haspopup="menu" onclick={(e) => (e.stopPropagation(), menu(e, t, plId))} ondblclick={(e) => e.stopPropagation()}><Icon n="more" size={18} /></button>
    </div>
  {:else}
    <p class="empty">{library.length ? tr('ไม่พบเพลง') : tr('ยังไม่มีเพลง')}</p>
  {/each}
  </div>
  {/if}
  </div>
{/snippet}

<div class="app" class:withly={barLy && !!current} class:compact class:noright={!panel} class:hasbg={!!look.bg} style:--tint={look.tint ? tint : look.base}>
  {#if look.bg}<div class="bgwrap" aria-hidden="true">{#if isVideo(look.bg)}<video class="bgimg bgvid" src={convertFileSrc(look.bg)} autoplay muted loop playsinline style:filter="blur({look.bgBlur}px)" use:noMotion></video>{:else}<div class="bgimg" style:background-image="url('{convertFileSrc(look.bg)}')" style:filter="blur({look.bgBlur}px)"></div>{/if}</div>{/if}
  <!-- แถบบน -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="nav" onmousedown={navDown}>
    <button class="logo" translate="no" aria-label="MySound" title="MySound" onclick={() => (view = 'home')}><span class="logomark" aria-hidden="true"><Icon n="wave" size={22} /></span></button>
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
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    {#if !compact}<div class="resizer r-side" role="separator" aria-orientation="vertical" aria-label={tr('ปรับความกว้างแถบซ้าย')} aria-valuenow={look.sidebar} aria-valuemin={180} aria-valuemax={320} tabindex="0" onpointerdown={(e) => resizer(e, 'sidebar')} onkeydown={(e) => resizeKey(e, 'sidebar')}></div>{/if}
    <div class="side-top">
      {#if !compact}<b>{tr('คลังเพลง')}</b>{/if}
      <button class="icon" title={tr('ย่อ/ขยาย')} aria-label={tr('ย่อหรือขยายคลังเพลง')} onclick={() => (compact = !compact)}><Icon n="sidebar" size={18} /></button>
      <button class="icon" title={tr('สร้างเพลย์ลิสต์')} aria-label={tr('สร้างเพลย์ลิสต์ใหม่')} onclick={() => (view = `pl:${newPlaylist()}`)}><Icon n="plus" size={18} /></button>
    </div>
    <nav>
      <button class="item" class:active={view === 'home'} onclick={() => (view = 'home')}><Icon n="home" /><span>{tr('หน้าแรก')}</span></button>
      <button class="item" class:active={view === 'songs'} onclick={() => (view = 'songs')}><Icon n="songs" /><span>{tr('เพลงทั้งหมด')}</span></button>
      <button class="item" class:active={view === 'download'} onclick={() => (view = 'download')}><Icon n="download" /><span>{tr('ดาวน์โหลด')}</span></button>
      <button class="item" class:active={view === 'artists' || view.startsWith('artist:') || view.startsWith('album:')} onclick={() => (view = 'artists')}><Icon n="artist" /><span>{tr('ศิลปินและอัลบั้ม')}</span></button>
      <button class="item" class:active={view === 'history'} onclick={() => (view = 'history')}><Icon n="history" /><span>{tr('ประวัติการฟัง')}</span></button>
      <button class="item" class:active={view === 'tools'} onclick={() => (view = 'tools')}><Icon n="tools" /><span>{tr('เครื่องมือ')}</span></button>
      <button class="item" class:active={view === 'look'} onclick={() => (view = 'look')}><Icon n="palette" /><span>{tr('หน้าตา')}</span></button>
    </nav>
    <hr />
    <div class="pls">
      {#each AUTO as a (a.id)}
        <button class="item" class:active={view === `auto:${a.id}`} onclick={() => (view = `auto:${a.id}`)}>
          <span class="thumb auto" style:background={tileColor(a.name)}><Icon n="wave" size={20} /></span>
          <span class="two"><b>{tr(a.name)}</b><small>{tr('อัตโนมัติ')}</small></span>
        </button>
      {/each}
      <hr />
      {#each smart as sp (sp.id)}
        <button class="item" class:active={view === `smart:${sp.id}`} onclick={() => (view = `smart:${sp.id}`)}>
          <span class="thumb auto" style:background={tileColor(sp.name)}><Icon n="wave" size={20} /></span>
          <span class="two"><b>{sp.name}</b><small>{tr('อัจฉริยะ')}</small></span>
        </button>
      {/each}
      {#if smart.length}<hr />{/if}
      {#snippet plItem(p: Playlist)}
        <button class="item" data-pl={p.id} class:dropon={drag?.overPl === p.id} class:active={view === `pl:${p.id}`} onclick={() => (view = `pl:${p.id}`)} oncontextmenu={(e) => plMenu(e, p)}>
          <span class="thumb auto" style:background={plTile(p)}>{#if !p.image}<Icon n="wave" size={20} />{/if}</span>
          <span class="two"><b>{p.pinned ? '📌 ' : ''}{p.name}</b><small>{tr('{0} เพลง', p.ids.length)}</small></span>
        </button>
      {/snippet}
      {#each playlists.filter((p) => p.pinned) as p (p.id)}{@render plItem(p)}{/each}
      {#each playlists.filter((p) => !p.pinned && !p.folder) as p (p.id)}{@render plItem(p)}{/each}
      {#each [...new Set(playlists.filter((p) => !p.pinned && p.folder).map((p) => p.folder!))] as f (f)}
        <details class="plfolder" open>
          <summary>📁 {f}</summary>
          {#each playlists.filter((p) => !p.pinned && p.folder === f) as p (p.id)}{@render plItem(p)}{/each}
        </details>
      {/each}
    </div>
  </aside>

  <!-- กลาง -->
  <main class="main panel" bind:this={mainEl} bind:clientHeight={viewH} onscroll={(e) => (scrollY = e.currentTarget.scrollTop)}>
  {#key view}<div class="vf">
    {#if selIds.length > 1}
      <div class="bulkbar" role="toolbar" aria-label={tr('จัดการเพลงที่เลือก')}>
        <b>{tr('เลือก {0} เพลง', selIds.length)}</b>
        <button class="ghost" onclick={() => selTracks[0] && play(selTracks[0], selTracks)}>{tr('เล่น')}</button>
        <button class="ghost" onclick={() => { const ids = [...selIds]; uq.push(...ids); say(tr('เพิ่ม {0} เพลงเข้าคิวแล้ว', ids.length), { label: tr('ย้อนกลับ'), fn: () => { for (const id of ids) { const i = uq.lastIndexOf(id); if (i >= 0) uq.splice(i, 1); } } }); }}>{tr('เพิ่มเข้าคิว')}</button>
        <select name="bulkpl" aria-label={tr('เพิ่มลงเพลย์ลิสต์')} onchange={(e) => { bulkToPlaylist(e.currentTarget.value); e.currentTarget.value = ''; }}>
          <option value="">{tr('เพิ่มลงเพลย์ลิสต์…')}</option>
          <option value="new">{tr('+ เพลย์ลิสต์ใหม่')}</option>
          {#each playlists as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
        </select>
        <select name="bulkrate" aria-label={tr('ให้ดาว')} onchange={(e) => { if (e.currentTarget.value) bulkRate(+e.currentTarget.value); e.currentTarget.value = ''; }}>
          <option value="">{tr('ให้ดาว…')}</option>
          {#each [5, 4, 3, 2, 1, 0] as n}<option value={n}>{n ? '★'.repeat(n) : tr('ล้างดาว')}</option>{/each}
        </select>
        <button class="ghost" onclick={() => bulkFav(true)}>{tr('ถูกใจ')}</button>
        <button class="ghost" onclick={openBulk}>{tr('แก้ข้อมูล…')}</button>
        <button class="ghost" disabled={!!coverBusy} onclick={bulkCovers}>{coverBusy || tr('หาปกให้')}</button>
        <button class="ghost" disabled={mp3Busy} onclick={() => toMp3(selTracks)}>{tr('แปลงเป็น MP3…')}</button>
        <button class="ghost danger" onclick={() => toTrash(selTracks)}>{tr('ลบ')}</button>
        <button class="ghost" onclick={() => (selIds = [])}>{tr('ยกเลิกเลือก')}</button>
      </div>
    {/if}
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
    {:else if view === 'history'}
      <h1>{tr('ประวัติการฟัง')}</h1>
      {#if !history.length}
        <p class="empty">{tr('ยังไม่มีประวัติ เล่นเพลงสักเพลงก่อน')}</p>
      {:else}
        <div class="hist">
          {#each history as h, i (h.at + ':' + i)}
            {@const t = library.find((x) => x.id === h.id)}
            {#if t}
              <button class="qrow" onclick={() => play(t, history.map((x) => library.find((y) => y.id === x.id)).filter((y): y is Track => !!y))} oncontextmenu={(e) => menu(e, t)}>
                <span class="thumb" style:background={art(t)}></span>
                <span class="two"><b>{t.title}</b><small>{tr(t.artist)}</small></span>
                <small class="dim">{new Date(h.at).toLocaleString(i18n.lang === 'th' ? 'th-TH' : 'en-US', { dateStyle: 'medium', timeStyle: 'short' })}</small>
              </button>
            {/if}
          {/each}
          <button class="ghost" onclick={() => (history = [])}>{tr('ล้างประวัติ')}</button>
        </div>
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
      {#if query.trim()}<button class="ghost" onclick={saveSmart}>{tr('บันทึกการค้นหานี้เป็นเพลย์ลิสต์อัจฉริยะ')}</button>{/if}
      <details class="qhelp"><summary>{tr('ค้นหาขั้นสูง')}</summary>
        <p class="dim">{tr('พิมพ์ตัวกรองรวมกับคำค้นได้ เช่น')} <code>skeleton rating:&gt;=4 added:&lt;7d tag:chill plays:&gt;3 dur:&gt;5m fav:yes artist:mandy album:"angel rage"</code></p>
      </details>
      {@render rows(shown)}
      {#if lyHits.length}
        <h2>{tr('พบในเนื้อเพลง')}</h2>
        <div class="hist">
          {#each lyHits as h (h.t.id)}
            <button class="qrow" onclick={() => playAtLine(h)}>
              <span class="thumb" style:background={art(h.t)}></span>
              <span class="two"><b>{h.t.title}</b><small>{tr(h.t.artist)} · “{h.line}”</small></span>
            </button>
          {/each}
        </div>
      {/if}
    {:else if smartCur}
      <h1>{smartCur.name}</h1>
      <p class="dim">{tr('เพลย์ลิสต์อัจฉริยะ · {0} เพลง', smartTracks.length)}</p>
      <div class="plbar">
        {#if smartTracks[0]}<button class="playall" aria-label={tr('เล่นทั้งหมด')} onclick={() => play(smartTracks[0], smartTracks)}><Icon n="play" size={22} /></button>{/if}
        <label class="smartq">{tr('ชื่อ')}<input name="smartname" autocomplete="off" bind:value={smartCur.name} /></label>
        <label class="smartq">{tr('เงื่อนไข')}<input name="smartq" autocomplete="off" spellcheck="false" bind:value={smartCur.q} /></label>
        <button class="ghost danger" onclick={() => { checkpoint(); smart = smart.filter((x) => x !== smartCur); view = 'songs'; }}>{tr('ลบ')}</button>
      </div>
      {@render rows(smartTracks)}
    {:else if view === 'artists'}
      <h1>{tr('ศิลปินและอัลบั้ม')}</h1>
      <h2>{tr('ศิลปิน')}</h2>
      <div class="cards">
        {#each artists as a (a.name)}
          <button class="card" onclick={() => (view = 'artist:' + a.name)}><div class="art round" style:background={art(a.ts[0])}></div><b>{tr(a.name)}</b><small>{tr('{0} เพลง', a.ts.length)}</small></button>
        {/each}
      </div>
      {#if albums.length}
        <h2>{tr('อัลบั้ม')}</h2>
        <div class="cards">
          {#each albums as a (a.name)}
            <button class="card" onclick={() => (view = 'album:' + a.name)}><div class="art" style:background={art(a.ts[0])}></div><b>{a.name}</b><small>{tr(a.ts[0].artist)} · {tr('{0} เพลง', a.ts.length)}</small></button>
          {/each}
        </div>
      {/if}
    {:else if artistName}
      <div class="plhead">
        <span class="art big round" style:background={artistTracks[0] ? art(artistTracks[0]) : cover(artistName)}></span>
        <div><small>{tr('ศิลปิน')}</small><h1>{tr(artistName)}</h1><p class="dim">{tr('{0} เพลง', artistTracks.length)}</p></div>
      </div>
      <div class="plbar">
        {#if artistTracks[0]}<button class="playall" aria-label={tr('เล่นทั้งหมด')} onclick={() => play(artistTracks[0], artistTracks)}><Icon n="play" size={22} /></button>{/if}
        {#each [...new Set(artistTracks.map((t) => t.album).filter(Boolean))] as al (al)}<button class="ghost" onclick={() => (view = 'album:' + al)}>{al}</button>{/each}
      </div>
      {@render rows(artistTracks)}
    {:else if albumName}
      <div class="plhead">
        <span class="art big" style:background={albumTracks[0] ? art(albumTracks[0]) : cover(albumName)}></span>
        <div><small>{tr('อัลบั้ม')}</small><h1>{albumName}</h1><p class="dim">{albumTracks[0] ? tr(albumTracks[0].artist) + ' · ' : ''}{tr('{0} เพลง', albumTracks.length)} · {fmt(albumTracks.reduce((a, t) => a + t.dur, 0))}</p></div>
      </div>
      {#if albumTracks[0]}<div class="plbar"><button class="playall" aria-label={tr('เล่นทั้งหมด')} onclick={() => play(albumTracks[0], albumTracks)}><Icon n="play" size={22} /></button></div>{/if}
      {@render rows(albumTracks)}
    {:else if view === 'trash'}
      <h1>{tr('ถังขยะ')}</h1>
      <p class="dim">{tr('เพลงที่ลบจะอยู่ที่นี่ 30 วัน แล้วลบไฟล์จริงอัตโนมัติ')}</p>
      {#if trash.length}
        <div class="plbar">
          <button class="ghost" onclick={() => restoreTrash(trash)}>{tr('กู้คืนทั้งหมด')}</button>
          <button class="ghost danger" onclick={() => purgeTrash(trash)}>{tr('ล้างถังขยะ')}</button>
        </div>
        <div class="hist">
          {#each trash as it (it.t.id)}
            <div class="qrow trow">
              <span class="thumb" style:background={art(it.t)}></span>
              <span class="two"><b>{it.t.title}</b><small>{tr(it.t.artist)} · {tr('เหลือ {0} วัน', Math.max(0, 30 - Math.floor((Date.now() - it.at) / 864e5)))}</small></span>
              <button class="ghost" onclick={() => restoreTrash([it])}>{tr('กู้คืน')}</button>
              <button class="ghost danger" onclick={() => purgeTrash([it])}>{tr('ลบถาวร')}</button>
            </div>
          {/each}
        </div>
      {:else}
        <p class="empty">{tr('ถังขยะว่าง')}</p>
      {/if}
    {:else if view === 'dups'}
      <h1>{tr('เพลงซ้ำ')}</h1>
      <p class="dim">{tr('จับจากชื่อเพลงและศิลปินที่เหมือนกัน (ไม่สนวงเล็บหรือคำว่า Official) และความยาวต่างกันไม่เกิน 3 วินาที เลือกอันที่จะเก็บไว้ ที่เหลือจะย้ายไปถังขยะ โดยรวมจำนวนครั้งที่เล่น ดาว และเพลย์ลิสต์ให้')}</p>
      {#each dups as g (g.map((t) => t.id).join('-'))}
        <div class="dupg">
          {#each g as t (t.id)}
            <div class="qrow trow">
              <span class="thumb" style:background={art(t)}></span>
              <span class="two"><b>{t.title}</b><small>{tr(t.artist)} · {fmt(t.dur)} · {tr('เล่น {0} ครั้ง', t.plays)}{t.cover ? '' : ' · ' + tr('ไม่มีปก')}</small></span>
              <button class="ghost" onclick={() => play(t)}>{tr('ฟัง')}</button>
              <button class="ghost" onclick={() => resolveDup(g, t)}>{tr('เก็บอันนี้')}</button>
            </div>
          {/each}
          <button class="link" onclick={() => (dupIgnore = [...dupIgnore, dupKey(g[0])])}>{tr('ไม่ใช่เพลงซ้ำ')}</button>
        </div>
      {:else}
        <p class="empty">{tr('ไม่พบเพลงซ้ำ')}</p>
      {/each}
    {:else if view === 'storage'}
      <h1>{tr('พื้นที่เก็บข้อมูล')}</h1>
      {#if store}
        <dl class="storedl">
          <dt>{tr('ไฟล์เพลง')}</dt><dd>{mb(store.audio)}</dd>
          <dt>{tr('ปกและรูปพื้นหลัง')}</dt><dd>{mb(store.covers)}</dd>
          <dt>{tr('ไฟล์อื่น')}</dt><dd>{mb(store.other)}</dd>
          <dt>{tr('ข้อมูลคลัง')}</dt><dd>{mb(store.library)}</dd>
          <dt>{tr('ไฟล์ที่ไม่มีเพลงไหนใช้')}</dt><dd>{tr('{0} ไฟล์', store.orphans.length)} · {mb(store.orphan_bytes)}</dd>
        </dl>
        <button class="ghost" disabled={!store.orphans.length} onclick={cleanOrphans}>{tr('ลบไฟล์ที่ไม่ได้ใช้')}</button>
        <button class="ghost" onclick={loadStore}>{tr('ตรวจใหม่')}</button>
      {:else}
        <p class="dim" role="status">{storeMsg || tr('กำลังตรวจ…')}</p>
      {/if}
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
      {#if spUser}
        <div class="spbox">
          <b>Spotify</b>
          <button class="ghost" onclick={spLoadPls}>{tr('เลือกเพลย์ลิสต์…')}</button>
          <button class="ghost" onclick={() => spImport('liked')}>{tr('เพลงที่ถูกใจ')}</button>
          <small class="dim">{tr('หรือวางลิงก์เพลย์ลิสต์/อัลบั้ม Spotify ในช่องด้านบน')}</small>
        </div>
        <p class="errtxt" role="status">{spMsg}</p>
        {#if spPls}
          <div class="sppls">
            {#each spPls as p (p.id)}
              <button class="sppl" disabled={!p.mine} title={p.mine ? p.name : tr('Spotify ไม่ให้อ่านเพลย์ลิสต์ของคนอื่น ก๊อปเพลงไปไว้ในเพลย์ลิสต์ของคุณก่อน')} onclick={() => { spPls = null; spImport(p.id); }}>
                {#if p.image}<img src={p.image} alt="" width="40" height="40" loading="lazy" />{:else}<span class="thumb"></span>{/if}
                <span><b>{p.name}</b><small class="dim">{p.mine ? tr('{0} เพลง', p.count) : tr('ของคนอื่น (อ่านไม่ได้)')}</small></span>
              </button>
            {/each}
          </div>
        {/if}
      {/if}
      <details class="pastebox">
        <summary>{tr('วางรายชื่อเพลง (ไม่ต้องใช้ Spotify)')}</summary>
        <textarea name="pastelist" rows="6" spellcheck="false" aria-label={tr('รายชื่อเพลง')} placeholder={tr('บรรทัดละเพลง เช่น\nThe Weeknd - Blinding Lights\nAtom ชนกันต์ - ทางของฝุ่น')} bind:value={pasteList}></textarea>
        <div class="pasterow">
          <input name="pastename" autocomplete="off" aria-label={tr('ชื่อเพลย์ลิสต์')} placeholder={tr('ชื่อเพลย์ลิสต์ (ไม่ใส่ก็ได้)…')} bind:value={pasteName} />
          <button class="primary" disabled={!pasteList.trim()} onclick={importPasted}>{tr('หาและโหลด {0} เพลง', parseList(pasteList).length)}</button>
        </div>
      </details>
      {#if spImp}
        {@const found = spImp.items.filter((x) => x.st !== 'wait' && x.st !== 'search').length}
        <div class="spimp">
          <div class="sphead">
            <b>{tr('นำเข้า “{0}”', spImp.name)}</b>
            <small class="dim">{tr('หาแล้ว {0}/{1}', found, spImp.items.length)} · {tr('ส่งโหลด {0}', spCount('ok'))} · {tr('มีอยู่แล้ว {0}', spCount('have'))}{#if spCount('pick') + spCount('none')}{' · '}<span class="errtxt">{tr('ต้องเลือกเอง {0}', spCount('pick') + spCount('none'))}</span>{/if}</small>
            {#if !spImp.done}<button class="ghost" onclick={() => spImp && (spImp.stop = true)}>{tr('หยุดค้น')}</button>{:else}<button class="ghost" onclick={() => (spImp = null)}>{tr('ปิด')}</button>{/if}
          </div>
          <div class="bar"><i style:width="{(found / spImp.items.length) * 100}%"></i></div>
          {#each spImp.items as it, i (i)}
            {#if it.st === 'pick' || it.st === 'none'}
              <div class="sppick">
                <span><b>{it.t.title}</b> <small class="dim">{it.t.artist}{it.t.dur ? ` · ${fmt(it.t.dur)}` : ''}</small></span>
                <div class="spalts">
                  {#if it.st === 'none'}<small class="dim">{tr('ไม่พบบน YouTube')}</small>{:else}<small class="dim">{tr('ไม่แน่ใจว่าตัวไหนใช่ เลือกเอง:')}</small>{/if}
                  {#each it.alts ?? [] as h (h.url)}
                    <button class="ghost" title={h.url} onclick={() => spQueue(it, h)}>{h.title} <small class="dim">{h.uploader} · {fmt(h.dur)}</small></button>
                  {/each}
                  <button class="ghost" onclick={() => (it.st = 'skip')}>{tr('ข้าม')}</button>
                </div>
              </div>
            {/if}
          {/each}
        </div>
      {/if}
      <h2>{tr('ค้นหาเพลง')}</h2>
      <form class="dlform" role="search" onsubmit={(e) => (e.preventDefault(), doSearch())}>
        <div class="seg wide srcseg" role="group" aria-label={tr('แหล่งที่ค้น')}>
          <button type="button" class:on={sSource === 'youtube'} aria-pressed={sSource === 'youtube'} onclick={() => (sSource = 'youtube')}>YouTube</button>
          <button type="button" class:on={sSource === 'soundcloud'} aria-pressed={sSource === 'soundcloud'} onclick={() => (sSource = 'soundcloud')}>SoundCloud</button>
        </div>
        <input type="search" name="song" autocomplete="off" spellcheck="false" aria-label={tr('ชื่อเพลงที่จะค้น')} placeholder={tr('พิมพ์ชื่อเพลงหรือศิลปิน…')} bind:value={sq} />
        <button class="primary" disabled={sBusy}>{sBusy ? tr('กำลังค้นหา…') : tr('ค้นหา')}</button>
      </form>
      {#if sErr}<p class="errtxt" role="alert">{sErr}</p>{/if}
      {#if searched && !sBusy && !hits.length && !sErr}<p class="empty">{tr('ไม่พบผลลัพธ์')}</p>{/if}
      <div aria-live="polite">
        {#if sBusy && !hits.length}<div class="skel rows" role="status" aria-label={tr('กำลังค้นหา…')}>{#each [1, 2, 3, 4, 5] as i (i)}<i></i>{/each}</div>{/if}
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
            <b>{d.playlist && d.state !== 'done' ? (d.n ? tr('{0} · เพลงที่ {1}/{2}', d.title, d.idx, d.n) : tr('เพลย์ลิสต์ · ได้แล้ว {0} เพลง', d.count)) : d.title}</b>
            <small class="dim">{d.src === 'youtube' ? 'YouTube' : 'SoundCloud'} · {tr(stateText[d.state])}{#if d.playlist && d.cur && d.state !== 'done'} · {d.cur}{/if}{#if d.playlist && d.count} · {tr('ได้แล้ว {0}', d.count)}{/if}</small>
            {#if d.msg}<small class="errtxt">{d.msg}</small>{/if}
            {#if d.playlist && d.state === 'done'}
              <small class:errtxt={!!d.skipped}>{d.n && d.count < d.n ? tr('ได้ {0} จาก {1} เพลง', d.count, d.n) : tr('ได้ครบ {0} เพลง', d.count)}{#if d.skipped}{' · '}{tr('ข้าม {0} เพลง', d.skipped)}{/if}</small>
              {#if d.why?.length}
                <details class="dlwhy"><summary>{tr('ดูสาเหตุที่ข้าม')}</summary><ul>{#each d.why as w (w.m)}<li>{w.m}{w.n > 1 ? ` × ${w.n}` : ''}</li>{/each}</ul></details>
              {/if}
            {/if}
            <div class="bar"><i style:width="{d.pct}%"></i></div>
          </div>
          {#if d.state === 'failed'}<button class="ghost" onclick={() => runJob(d)}>{tr('ลองใหม่')}</button>{:else if d.playlist && d.state === 'done' && d.skipped}<button class="ghost" title={tr('เพลงที่มีอยู่แล้วจะไม่โหลดซ้ำ')} onclick={() => runJob(d)}>{tr('ลองโหลดที่ข้ามอีกรอบ')}</button>{:else}<span class="dim num-tab">{Math.round(d.pct)}%</span>{/if}
        </div>
      {/each}
      </div>
    {:else if view === 'tools'}
      <h1>{tr('เครื่องมือ')}</h1>
      <div class="tooltabs" role="tablist" aria-label={tr('หมวดเครื่องมือ')}>
        {#each TOOL_TABS as [k, label] (k)}
          <button type="button" role="tab" aria-selected={toolTab === k} class:on={toolTab === k} onclick={() => (toolTab = k)}>{tr(label)}</button>
        {/each}
      </div>
      {#if toolTab === 'gen'}
      <div class="tool">
        <div><b>{tr('อัปเดตแอป')}</b><small class="dim">{tr('เช็กเวอร์ชันใหม่จาก GitHub ตอนเปิดแอปอยู่แล้ว กดตรงนี้เพื่อเช็กเอง')}</small></div>
        {#if upd}
          <button class="ghost" disabled={updBusy} onclick={() => installUpdate()}>{tr('อัปเดตเป็น {0}', upd.version)}</button>
        {:else}
          <button class="ghost" disabled={updBusy} onclick={() => checkUpdate(true)}>{tr('ตรวจหาเวอร์ชันใหม่')}</button>
        {/if}
      </div>
      <p class="dim" role="status" aria-live="polite">{updMsg}</p>
      <h2>{tr('Windows')}</h2>
      <div class="tool">
        <div><b>{tr('ปิดหน้าต่างแล้วย่อไปถาดระบบ')}</b><small class="dim">{tr('กดปิดแล้วเพลงยังเล่นต่อ คลิกไอคอนที่มุมขวาล่างเพื่อเปิดกลับ คลิกขวาที่ไอคอนเพื่อควบคุมเพลงหรือออกจากแอป')}</small></div>
        <label class="switch"><input type="checkbox" name="closetotray" bind:checked={closeToTray} /><span>{closeToTray ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <div class="tool">
        <div><b>{tr('ปุ่มลัดทั้งระบบ')}</b><small class="dim">{tr('ใช้ได้แม้อยู่ในเกมหรือแอปอื่น: Ctrl+Alt+Space เล่น/หยุด · Ctrl+Alt+→/← ถัดไป/ก่อนหน้า · Ctrl+Alt+↑/↓ เสียง')}</small></div>
        <label class="switch"><input type="checkbox" name="hotkeys" bind:checked={hotkeys} /><span>{hotkeys ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <div class="tool">
        <div><b>{tr('เปิดพร้อม Windows')}</b><small class="dim">{tr('เปิดแอปเองตอนเข้าเครื่อง แบบย่ออยู่ในถาดระบบ')}</small></div>
        <label class="switch"><input type="checkbox" name="autostart" checked={autoStart} onchange={(e) => setAutoStart(e.currentTarget.checked)} /><span>{autoStart ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <div class="tool">
        <div><b>{tr('แจ้งเตือนตอนเปลี่ยนเพลง')}</b><small class="dim">{tr('ขึ้นแจ้งเตือนของ Windows เฉพาะตอนที่ไม่ได้เปิดแอปอยู่หน้าสุด')}</small></div>
        <label class="switch"><input type="checkbox" name="notify" bind:checked={notifyOn} /><span>{notifyOn ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <div class="tool">
        <div><b>{tr('แถบความคืบหน้าบนแถบงาน')}</b><small class="dim">{tr('ไอคอน MySound บนแถบงานแสดงว่าเพลงเล่นไปถึงไหนแล้ว')}</small></div>
        <label class="switch"><input type="checkbox" name="taskprog" bind:checked={taskProg} /><span>{taskProg ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      {/if}
      {#if toolTab === 'gen'}
      <h2>{tr('อัตโนมัติ')}</h2>
      <div class="tool">
        <div><b>{tr('นาฬิกาปลุก')}</b><small class="dim">{tr('เล่นเพลย์ลิสต์ตามเวลาทุกวัน (แอปต้องเปิดอยู่หรือย่ออยู่ในถาดระบบ)')}</small></div>
        <input type="time" name="alarmtime" aria-label={tr('เวลาปลุก')} bind:value={alarm.time} />
        <select name="alarmpl" aria-label={tr('เพลย์ลิสต์ที่จะเล่น')} bind:value={alarm.pl}>
          <option value="">{tr('ทั้งคลัง')}</option>
          {#each playlists as p (p.id)}<option value={String(p.id)}>{p.name}</option>{/each}
        </select>
        <label class="switch"><input type="checkbox" name="alarm" bind:checked={alarm.on} /><span>{alarm.on ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      {#if alarm.on}<label class="switch sub"><input type="checkbox" name="alarmramp" bind:checked={alarm.ramp} /><span>{tr('ค่อย ๆ เพิ่มเสียงใน 1 นาที')}</span></label>{/if}
      <div class="tool">
        <div><b>{tr('ติดตามช่อง/เพลย์ลิสต์')}</b><small class="dim">{tr('ตรวจทุก 6 ชั่วโมง ถ้ามีเพลงใหม่จะโหลดให้เอง')}</small></div>
        <input type="url" name="followurl" autocomplete="off" spellcheck="false" placeholder={tr('ลิงก์ช่องหรือเพลย์ลิสต์…')} bind:value={followUrl} />
        <button class="ghost" disabled={!followUrl.trim()} onclick={addFollow}>{tr('ติดตาม')}</button>
        {#if follows.length}<button class="ghost" onclick={() => checkFollows(true)}>{tr('ตรวจตอนนี้')}</button>{/if}
      </div>
      {#each follows as f, i (f.url)}
        <div class="tool sub"><div><b>{f.name}</b><small class="dim">{f.url}</small></div><button class="ghost" onclick={() => follows.splice(i, 1)}>{tr('เลิกติดตาม')}</button></div>
      {/each}
      <p class="dim" role="status" aria-live="polite">{followMsg}</p>
      <div class="tool">
        <div><b>{tr('อัปเดต yt-dlp ให้เอง')}</b><small class="dim">{tr('เมื่อเวอร์ชันเก่ากว่า 14 วัน (ช่วยให้โหลดจาก YouTube ได้ต่อเนื่อง)')}</small></div>
        <label class="switch"><input type="checkbox" name="autoyt" bind:checked={autoYt} /><span>{autoYt ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <div class="tool">
        <div><b>{tr('ติดตั้งอัปเดตแอปให้เอง')}</b><small class="dim">{tr('เมื่อมีเวอร์ชันใหม่และไม่ได้เล่นเพลงอยู่ จะติดตั้งแบบเงียบแล้วเปิดแอปใหม่')}</small></div>
        <label class="switch"><input type="checkbox" name="autoupd" bind:checked={autoUpd} /><span>{autoUpd ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      {/if}
      {#if toolTab === 'net'}
      <h2>Spotify</h2>
      <div class="tool">
        <div><b>{tr('นำเข้าเพลย์ลิสต์จาก Spotify')} <span class="badge">{tr('ต้องใช้ Spotify Premium')}</span></b><small class="dim">{spUser ? tr('เชื่อมกับ {0} แล้ว · ไปที่หน้าดาวน์โหลดเพื่อเลือกเพลย์ลิสต์ หรือวางลิงก์เพลย์ลิสต์/อัลบั้ม Spotify ในช่องโหลดเพลง', spUser) : tr('อ่านรายชื่อเพลงจากเพลย์ลิสต์/อัลบั้มใน Spotify แล้วหาเพลงเดียวกันบน YouTube มาโหลด (ไม่ได้โหลดเสียงจาก Spotify) · Spotify ให้อ่านได้เฉพาะเพลย์ลิสต์ที่คุณสร้างหรือร่วมแก้ไข')}</small></div>
        {#if spUser}
          <button class="ghost" onclick={() => (view = 'download')}>{tr('ไปหน้าดาวน์โหลด')}</button>
          <button class="ghost" onclick={spDisconnect}>{tr('ตัดการเชื่อมต่อ')}</button>
        {:else}
          <button class="ghost" disabled={spBusy || !spCid.trim()} onclick={spConnect}>{spBusy ? tr('รอล็อกอินในเบราว์เซอร์…') : tr('เชื่อม Spotify')}</button>
        {/if}
      </div>
      {#if !spUser}
        <div class="spsetup">
          <label>Client ID <input name="spcid" autocomplete="off" spellcheck="false" placeholder={tr('วาง Client ID 32 ตัวอักษร…')} bind:value={spCid} /></label>
          <details>
            <summary>{tr('วิธีสร้าง Client ID (ทำครั้งเดียว ประมาณ 2 นาที)')}</summary>
            <ol>
              <li>{tr('ไปที่')} <button class="alink" onclick={() => invoke('open_url', { url: 'https://developer.spotify.com/dashboard' })}>developer.spotify.com/dashboard</button> {tr('แล้วล็อกอินด้วยบัญชี Spotify Premium')}</li>
              <li>{tr('กด Create app ตั้งชื่ออะไรก็ได้ ช่อง Redirect URIs ใส่')} <code>http://127.0.0.1:43827/callback</code> <button class="ghost sm" onclick={() => { navigator.clipboard.writeText('http://127.0.0.1:43827/callback'); say(tr('คัดลอกแล้ว')); }}>{tr('คัดลอก')}</button> {tr('ติ๊ก Web API แล้วกด Save')}</li>
              <li>{tr('คัดลอก Client ID จากหน้าแอปมาวางในช่องด้านบน แล้วกด “เชื่อม Spotify”')}</li>
            </ol>
          </details>
        </div>
      {/if}
      <p class="errtxt" role="status">{spMsg}</p>
      <h2>{tr('เชื่อมต่อ')}</h2>
      <div class="tool">
        <div><b>{tr('รีโมทผ่านมือถือ')}</b><small class="dim">{tr('เปิดลิงก์บนมือถือที่ต่อ Wi‑Fi เดียวกัน เพื่อสั่งเล่น หยุด ข้าม ปรับเสียง และเลือกเพลง (Windows อาจถามให้อนุญาตผ่านไฟร์วอลล์)')}</small></div>
        <label class="switch"><input type="checkbox" name="remote" checked={remoteOn} onchange={(e) => setRemote(e.currentTarget.checked)} /><span>{remoteOn ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      {#if remoteInfo}<p class="remoteurl">{#each remoteInfo.urls as u (u)}<code>{u}</code>{/each}<small class="dim">{tr('PIN อยู่ในลิงก์แล้ว ({0}) อย่าแชร์ลิงก์นี้ให้คนอื่น', remoteInfo.pin)}</small></p>{/if}
      <p class="errtxt" role="status">{remoteMsg}</p>
      <div class="tool">
        <div><b>Last.fm</b><small class="dim">{lfm.sk ? tr('เชื่อมต่อแล้ว: {0} · บันทึกเพลงที่ฟังเกินครึ่งเพลงหรือ 4 นาที', lfm.user) : tr('บันทึกเพลงที่ฟัง (scrobble) เข้าบัญชี Last.fm ต้องสร้าง API key เองที่ last.fm/api/account/create')}</small></div>
        {#if lfm.sk}
          <label class="switch"><input type="checkbox" name="lfmon" bind:checked={lfm.on} /><span>{lfm.on ? tr('เปิด') : tr('ปิด')}</span></label>
          <button class="ghost" onclick={() => (lfm = { ...lfm, sk: '', user: '', on: false })}>{tr('ตัดการเชื่อมต่อ')}</button>
        {/if}
      </div>
      {#if !lfm.sk}
        <div class="tool sub">
          <input name="lfmkey" autocomplete="off" spellcheck="false" placeholder="API key" bind:value={lfm.key} />
          <input name="lfmsecret" type="password" autocomplete="off" spellcheck="false" placeholder="Shared secret" bind:value={lfm.secret} />
          <button class="ghost" onclick={lfmConnect}>{tr('เชื่อมต่อ')}</button>
          <button class="ghost" onclick={lfmConfirm}>{tr('ยืนยัน')}</button>
        </div>
      {/if}
      <p class="dim" role="status" aria-live="polite">{lfmMsg}</p>
      {/if}
      {#if toolTab === 'lib'}
      <h2>{tr('นำเข้าและสำรองข้อมูล')}</h2>
      <div class="tool">
        <div><b>{tr('นำเข้าไฟล์เพลงจากเครื่อง')}</b><small class="dim">{tr('หรือลากไฟล์มาวางในหน้าต่างแอป อ่านชื่อ ศิลปิน อัลบั้ม และปกจากแท็กในไฟล์ (ไฟล์จะถูกคัดลอกเข้าแอป)')}</small></div>
        <button class="ghost" onclick={pickImport}>{tr('เลือกไฟล์…')}</button>
      </div>
      <div class="tool">
        <div><b>{tr('โฟลเดอร์เฝ้าดู')}</b><small class="dim">{watchDir ? tr('เฝ้าดู: {0} (ตรวจทุก 1 นาที)', watchDir) : tr('ไฟล์เพลงใหม่ที่วางในโฟลเดอร์นี้จะถูกนำเข้าเอง')}</small></div>
        <button class="ghost" onclick={pickWatch}>{watchDir ? tr('เปลี่ยน…') : tr('เลือกโฟลเดอร์…')}</button>
        {#if watchDir}<button class="ghost" onclick={() => ((watchDir = ''), (watchSeen = []))}>{tr('เลิกเฝ้าดู')}</button>{/if}
      </div>
      <div class="tool">
        <div><b>{tr('นำเข้าเพลย์ลิสต์')}</b><small class="dim">{tr('ไฟล์ .m3u/.m3u8, ไฟล์ .mysound ที่เพื่อนแชร์มา หรือไฟล์ CSV จาก Spotify (ส่งออกด้วยเว็บ Exportify) เพลงที่ไม่มีในคลังจะค้นบน YouTube แล้วโหลดให้')}</small></div>
        <button class="ghost" disabled={!!csvJob} onclick={pickListFile}>{csvJob ? `${csvJob.done}/${csvJob.total}` : tr('เลือกไฟล์…')}</button>
      </div>
      <div class="tool">
        <div><b>{tr('สำรอง/กู้คืนคลังเพลง')}</b><small class="dim">{tr('บันทึกรายชื่อเพลง ดาว เพลย์ลิสต์ และประวัติเป็นไฟล์เดียว (ไม่รวมไฟล์เสียง)')}</small></div>
        <button class="ghost" onclick={backup}>{tr('สำรอง…')}</button>
        <button class="ghost" onclick={restore}>{tr('กู้คืน…')}</button>
      </div>
      <p class="dim" role="status" aria-live="polite">{fileMsg}</p>
      {/if}
      {#if toolTab === 'lib'}
      <h2>{tr('จัดการคลัง')}</h2>
      <div class="tool">
        <div><b>{tr('เพลงซ้ำ')}</b><small class="dim">{tr('หาเพลงที่โหลดมาซ้ำ แล้วเลือกเก็บอันเดียว')}</small></div>
        <button class="ghost" onclick={() => (view = 'dups')}>{tr('ตรวจเพลงซ้ำ')}</button>
      </div>
      <div class="tool">
        <div><b>{tr('ถังขยะ')}</b><small class="dim">{tr('{0} เพลง · กู้คืนได้ภายใน 30 วัน', trash.length)}</small></div>
        <button class="ghost" onclick={() => (view = 'trash')}>{tr('เปิดถังขยะ')}</button>
      </div>
      <div class="tool">
        <div><b>{tr('พื้นที่เก็บข้อมูล')}</b><small class="dim">{tr('ดูว่าใช้พื้นที่ไปเท่าไร และลบไฟล์ที่ไม่ได้ใช้')}</small></div>
        <button class="ghost" onclick={() => ((view = 'storage'), loadStore())}>{tr('เปิดดู')}</button>
      </div>
      <div class="tool">
        <div><b>{tr('ซ่อมไฟล์ที่หาย')}</b><small class="dim">{tr('{0} เพลงหาไฟล์ไม่เจอ ถ้าย้ายไฟล์ไปไว้ที่อื่น ให้ชี้โฟลเดอร์นั้น แอปจะค้นและเชื่อมกลับให้', missing.length)}</small></div>
        <button class="ghost" disabled={!missing.length} onclick={repairMissing}>{tr('เลือกโฟลเดอร์…')}</button>
      </div>
      <p class="dim" role="status" aria-live="polite">{relinkMsg}</p>
      {/if}
      {#if toolTab === 'lyr'}
      <h2>{tr('เนื้อเพลง')}</h2>
      <div class="tool">
        <div><b>{tr('โหลดเนื้อเพลงทั้งคลัง')}</b><small class="dim">{tr('เก็บไว้ในเครื่อง ดูได้แม้ไม่มีเน็ต และใช้ค้นหาจากเนื้อเพลงได้ (มีแล้ว {0} เพลง)', lyCount)}</small></div>
        {#if pre}
          <button class="ghost" onclick={() => (preStop = true)}>{tr('หยุด')} {pre.done}/{pre.total}</button>
        {:else}
          <button class="ghost" disabled={!library.length} onclick={prefetchLyrics}>{tr('เริ่มโหลด')}</button>
        {/if}
      </div>
      <div class="tool">
        <div><b>{tr('ส่งออกเนื้อเพลงเป็นไฟล์ .lrc')}</b><small class="dim">{tr('เฉพาะเพลงที่มีเนื้อแบบมีเวลา ตั้งชื่อไฟล์เป็น “ศิลปิน - ชื่อเพลง.lrc”')}</small></div>
        <button class="ghost" onclick={exportLrc}>{tr('เลือกโฟลเดอร์แล้วส่งออก…')}</button>
      </div>
      <p class="dim" role="status" aria-live="polite">{lrcMsg}</p>
      <div class="tool">
        <div><b>{tr('ย้อนกลับ/ทำซ้ำ')}</b><small class="dim">{tr('Ctrl+Z ย้อนการแก้ไขคลังล่าสุด (ลบ แก้ข้อมูล ให้ดาวหลายเพลง ฯลฯ) Ctrl+Y ทำซ้ำ')}</small></div>
        <button class="ghost" onclick={undo}>{tr('ย้อนกลับ')}</button>
        <button class="ghost" onclick={redo}>{tr('ทำซ้ำ')}</button>
      </div>
      {/if}
      {#if toolTab === 'char'}
      <h2>{tr('ตัวละครบนหน้าจอ')}</h2>
      <div class="tool">
        <div><b>{tr('แสดงตัวละคร')}</b><small class="dim">{!pet ? tr('เพิ่มตัวละครก่อน แล้วใส่รูปให้ครบ 3 แบบ') : petReady ? tr('ลอยอยู่บนหน้าจอเสมอ ย่อแอปแล้วก็ยังอยู่ · คลิกขวาที่ตัวละครเพื่อล็อก ย้ายมุม หรือซ่อน') : tr('ยังขาดรูป: {0}', petMissing(pet).join(', '))}</small></div>
        <label class="switch"><input type="checkbox" name="peton" bind:checked={petOn} disabled={!petReady} /><span>{petOn && petReady ? tr('เปิด') : tr('ปิดอยู่')}</span></label>
      </div>
      <div class="petlist" role="listbox" aria-label={tr('ตัวละครที่บันทึกไว้')}>
        {#each pets as p (p.id)}
          <button class="petchip" class:on={p.id === petId} role="option" aria-selected={p.id === petId} onclick={() => (petId = p.id)}>
            <span class="petthumb checker">{#if p.idle}<img src={convertFileSrc(p.idle)} alt="" width="40" height="40" />{/if}</span>
            <span>{p.name}</span>
          </button>
        {/each}
        <button class="petchip add" onclick={addPet}><span class="petthumb">+</span><span>{tr('เพิ่มตัวละคร')}</span></button>
      </div>
      {#if pet}
        <div class="petedit">
          <label class="petname">{tr('ชื่อ')}<input name="petname" autocomplete="off" maxlength="32" bind:value={pet.name} /></label>
          <div class="petslots">
            {#each PET_SLOTS as [k, label], i (k)}
              <div class="petslot" class:missing={!pet[k]}>
                <div class="petimg checker">{#if pet[k]}<img src={convertFileSrc(pet[k])} alt={tr(label)} />{:else}<span class="dim">{tr('ยังไม่มีรูป')}</span>{/if}</div>
                <b>{i + 1} · {tr(label)}</b>
                <button class="ghost" onclick={() => pickSprite(k)}>{pet[k] ? tr('เปลี่ยนรูป') : tr('เลือกรูป')}</button>
              </div>
            {/each}
          </div>
          <small class="dim">{tr('รองรับ PNG (พื้นโปร่งใส), GIF และ WebP แบบขยับได้ ไม่เกิน 20 MB · ส่วนที่โปร่งใสของรูปคลิกทะลุไปโปรแกรมข้างหลังได้')}</small>
          {#if petReady}
            <div class="pettry">
              <span class="dim">{tr('ลองชี้และคลิก:')}</span>
              <span class="petdemo checker" tabindex="-1"><img class="s1" src={convertFileSrc(pet.idle)} alt="" /><img class="s2" src={convertFileSrc(pet.hover)} alt="" /><img class="s3" src={convertFileSrc(pet.press)} alt="" /></span>
            </div>
          {/if}
          <div class="petrow">
            <label class="petsize"><span>{tr('ขนาด')} <b>{pet.size}px</b></span><input type="range" min="48" max="800" step="4" bind:value={pet.size} disabled={pet.locked} aria-label={tr('ขนาดตัวละคร')} /></label>
            <div class="seg wide" role="group" aria-label={tr('ย้ายไปมุมจอ')}>
              <button type="button" disabled={!petOn || pet.locked} onclick={() => emit('pet-snap', 'left')}>{tr('มุมซ้ายล่าง')}</button>
              <button type="button" disabled={!petOn || pet.locked} onclick={() => emit('pet-snap', 'right')}>{tr('มุมขวาล่าง')}</button>
            </div>
          </div>
          <label class="switch"><input type="checkbox" name="petlock" bind:checked={pet.locked} /><span>{tr('ล็อกตำแหน่งและขนาด (ยังชี้/คลิกตัวละครได้)')}</span></label>
          <div class="seg wide" role="group" aria-label={tr('โหมดตัวละคร')}>
            <button type="button" class:on={!petBounce} aria-pressed={!petBounce} onclick={() => (petBounce = false)}>{tr('แบบปกติ')}</button>
            <button type="button" class:on={petBounce} aria-pressed={petBounce} onclick={() => (petBounce = true)}>{tr('เด้งตามเพลง')}</button>
          </div>
          {#if petBounce}<small class="dim">{tr('เด้งตามความดังของเพลงตอนเล่น (ถ้าอ่านเสียงไม่ได้จะเด้งตามจังหวะ BPM แทน) หยุดเพลงแล้วหยุดเด้ง')}</small>{/if}
          <label class="switch"><input type="checkbox" name="petlife" bind:checked={petLife} /><span>{tr('ท่าทางมีชีวิต: หายใจตอนหยุดเพลง, หลับเมื่อหยุดนาน, หัวใจลอยตอนกดถูกใจ')}</span></label>
          <label class="switch"><input type="checkbox" name="petly" bind:checked={petLy} /><span>{tr('แสดงเนื้อเพลงบรรทัดที่กำลังร้องเหนือตัวละคร')}</span></label>
          <p class="errtxt" role="status">{petMsg}</p>
          <button class="ghost danger" onclick={() => delPet(pet!)}>{tr('ลบตัวละครนี้')}</button>
        </div>
      {/if}
      {/if}
      {#if toolTab === 'keys'}
      <p class="dim">{tr('ปุ่มเหล่านี้ไม่ทำงานตอนกำลังพิมพ์ในช่องข้อความ')}</p>
      {#each KEYS as g (g.title)}
        <h2>{tr(g.title)}</h2>
        {#if g.note}<p class="dim">{tr(g.note)}</p>{/if}
        <dl class="keylist">
          {#each g.rows as [k, d] (k)}
            <dt><kbd>{k}</kbd></dt><dd>{tr(d)}</dd>
          {/each}
        </dl>
      {/each}
      {/if}
      {#if toolTab === 'sys'}
      <h2>{tr('โปรแกรมที่แอปเรียกใช้')}</h2>
      <div class="tool">
        <div><b>yt-dlp</b><small class="dim">{ytVer === undefined ? tr('กำลังตรวจ…') : ytVer === null ? tr('ไม่พบ — winget install yt-dlp.yt-dlp') : tr('เวอร์ชัน {0}', ytVer) + (ytAge !== null ? tr(' ({0} วันที่แล้ว)', ytAge) : '')}</small></div>
        <button class="ghost" disabled={toolBusy} onclick={updateYtdlp}>{tr('อัปเดต yt-dlp')}</button>
      </div>
      <div class="tool">
        <div><b>ffmpeg</b><small class="dim">{ffVer === undefined ? tr('กำลังตรวจ…') : ffVer === null ? tr('ไม่พบ (ไม่บังคับ ใช้กับการปรับเสียงและฝังแท็กตอนส่งออก) — winget install Gyan.FFmpeg') : tr('เวอร์ชัน {0}', ffVer)}</small></div>
        <button class="ghost" onclick={refreshTools}>{tr('ตรวจใหม่')}</button>
      </div>
      <div class="tool">
        <div><b>{tr('แสดงเนื้อเพลงในแถบเล่นด้านล่าง')}</b><small class="dim">{tr('บรรทัดที่กำลังร้องจะขึ้นใต้แถบเลื่อนเวลา (ต้องเป็นเพลงที่มีเนื้อแบบมีเวลา)')}</small></div>
        <label class="switch"><input type="checkbox" name="barly" bind:checked={barLy} /><span>{barLy ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      {/if}
      {#if toolTab === 'snd'}
      <h2>{tr('เอฟเฟกต์เสียง')}</h2>
      <div class="tool">
        <div><b>{tr('อีควอไลเซอร์')}</b><small class="dim">{tr('ปรับเบส/แหลม 10 แบนด์ เปิดแล้วแอปจะส่งเสียงผ่านตัวประมวลผลเสียง')}</small></div>
        <select name="eqpreset" aria-label={tr('ค่าสำเร็จรูป')} value={eqPreset} disabled={!eqOn} onchange={(e) => pickPreset(e.currentTarget.value)}>
          <option value="flat">{tr('ปกติ')}</option><option value="bass">{tr('เบสหนัก')}</option><option value="treble">{tr('แหลมใส')}</option>
          <option value="vocal">{tr('เสียงร้อง')}</option><option value="loudness">{tr('ลาวด์เนส')}</option><option value="rock">{tr('ร็อก')}</option><option value="pop">{tr('ป๊อป')}</option>
          {#if !EQ_PRESETS[eqPreset]}<option value={eqPreset}>{tr('กำหนดเอง')}</option>{/if}
        </select>
        <label class="switch"><input type="checkbox" name="eq" bind:checked={eqOn} /><span>{eqOn ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      {#if eqOn}
        <div class="eqbands" role="group" aria-label={tr('อีควอไลเซอร์')}>
          {#each EQ_FREQS as f, i (f)}
            <label><input type="range" name={'eq' + f} min="-12" max="12" step="1" value={eqGains[i]} oninput={(e) => { eqGains[i] = +e.currentTarget.value; eqPreset = 'custom'; }} aria-label={f + ' Hz'} /><small>{f >= 1000 ? f / 1000 + 'k' : f}</small></label>
          {/each}
        </div>
      {/if}
      <div class="tool balbox">
        <div class="balhead">
          <div><b id="bal-h">{tr('บาลานซ์ซ้าย–ขวา')}</b><small class="dim">{tr('เลื่อนไปทางที่อยากให้เสียงดังกว่า (ลำโพงหรือหูฟังข้างนั้น) กลางคือสมดุลปกติ')}</small></div>
          <output class="balval" class:mid={balance === 0} aria-live="polite">{balance === 0 ? tr('กลาง') : balance < 0 ? tr('ซ้าย {0}%', Math.round(-balance * 100)) : tr('ขวา {0}%', Math.round(balance * 100))}</output>
          <button type="button" class="ghost" disabled={balance === 0} onclick={() => (balance = 0)}>{tr('รีเซ็ต')}</button>
        </div>
        <div class="balbar">
          <span class="balend" aria-hidden="true"><b>L</b>{tr('ซ้าย')}</span>
          <div class="balrange">
            <i class="balmid" aria-hidden="true"></i>
            <input type="range" name="balance" min="-1" max="1" step="0.1" bind:value={balance} style:--lo="{Math.min(50, (balance + 1) * 50)}%" style:--hi="{Math.max(50, (balance + 1) * 50)}%" aria-labelledby="bal-h" aria-valuetext={balance === 0 ? tr('กลาง') : balance < 0 ? tr('ซ้าย {0}%', Math.round(-balance * 100)) : tr('ขวา {0}%', Math.round(balance * 100))} ondblclick={() => (balance = 0)} />
          </div>
          <span class="balend" aria-hidden="true"><b>R</b>{tr('ขวา')}</span>
        </div>
      </div>
      <div class="tool">
        <div><b>{tr('โมโน')}</b><small class="dim">{tr('ผสมเสียงซ้ายและขวาเป็นช่องเดียวกัน เหมาะกับคนที่ใช้หูฟังข้างเดียว หรือหูสองข้างได้ยินไม่เท่ากัน')}</small></div>
        <label class="switch"><input type="checkbox" name="mono" bind:checked={mono} /><span>{mono ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <div class="tool">
        <div><b>{tr('ครอสเฟด')}</b><small class="dim">{tr('เพลงเก่าค่อย ๆ เบาลงขณะที่เพลงถัดไปค่อย ๆ ดังขึ้น')}</small></div>
        <select name="xfade" aria-label={tr('ครอสเฟด')} bind:value={xfade}>
          <option value={0}>{tr('ปิด')}</option>
          {#each [2, 4, 6, 8, 12] as n}<option value={n}>{tr('{0} วินาที', n)}</option>{/each}
        </select>
      </div>
      <div class="tool">
        <div><b>{tr('เล่นต่อเนื่อง (Gapless)')}</b><small class="dim">{tr('ต่อเพลงถัดไปทันทีโดยไม่มีช่องว่าง (ถ้าตั้งครอสเฟดไว้ จะใช้ครอสเฟดแทน)')}</small></div>
        <label class="switch"><input type="checkbox" name="gapless" bind:checked={gapless} /><span>{gapless ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <div class="tool">
        <div><b>{tr('เบาเข้า/ออกตอนกดหยุดและเล่น')}</b><small class="dim">{tr('กดหยุดหรือเล่นแล้วเสียงค่อย ๆ เปลี่ยนแทนที่จะตัดทันที')}</small></div>
        <label class="switch"><input type="checkbox" name="softfade" bind:checked={softFade} /><span>{softFade ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <p class="errtxt" role="status" aria-live="polite">{fxMsg}</p>
      <div class="tool">
        <div><b>{tr('แยกชื่อศิลปินออกจากชื่อเพลง')}</b><small class="dim">{tr('ชื่อแบบ “ศิลปิน - ชื่อเพลง” จะถูกแยกไปใส่ช่องศิลปิน และเหลือแค่ชื่อเพลงในช่องชื่อ (เพลงที่โหลดใหม่ทำให้อัตโนมัติอยู่แล้ว)')}</small></div>
        <button class="ghost" disabled={!library.length} onclick={tidyTitles}>{tr('แก้ทั้งคลัง')}</button>
      </div>
      <p class="dim" role="status" aria-live="polite">{tidyMsg}</p>
      <div class="tool">
        <div><b>{tr('ปรับระดับเสียงแต่ละเพลงให้เท่ากัน')}</b><small class="dim">{tr('วัดความดังของแต่ละเพลงตอนเล่นครั้งแรก แล้วลดเพลงที่ดังเกินให้ใกล้เคียงกัน (เพิ่มเสียงเพลงที่เบาไม่ได้ ถ้าเบาไปให้เร่งปุ่มเสียงเอา)')}</small></div>
        <label class="switch"><input type="checkbox" name="normalize" bind:checked={normalize} /><span>{normalize ? tr('เปิด') : tr('ปิด')}</span></label>
        <button class="ghost" disabled={!!analyzing || !library.some((t) => t.lufs == null || t.bpm === undefined)} onclick={analyzeAll}>{analyzing ? `${analyzing.done}/${analyzing.total}` : tr('วิเคราะห์ทั้งคลัง')}</button>
      </div>
      <div class="tool">
        <div><b>{tr('คุกกี้สำหรับ YouTube')}</b><small class="dim">{tr('ถ้าดาวน์โหลดแล้วขึ้น “Sign in to confirm you’re not a bot” ให้เลือกเบราว์เซอร์ที่ล็อกอิน YouTube ไว้ Firefox เสถียรที่สุด ส่วน Chrome/Edge ต้องปิดเบราว์เซอร์ก่อนดาวน์โหลด และบางเวอร์ชันอ่านไม่ได้ ให้ใช้ไฟล์ cookies.txt แทน')}</small></div>
        <select name="ytcookies" aria-label={tr('คุกกี้สำหรับ YouTube')} value={ytCookies.startsWith('file:') ? 'file' : ytCookies} onchange={(e) => (ytCookies = e.currentTarget.value === 'file' ? 'file:' : e.currentTarget.value)}>
          <option value="">{tr('ไม่ใช้')}</option>
          <option value="firefox">Firefox</option>
          <option value="edge">Edge</option>
          <option value="chrome">Chrome</option>
          <option value="brave">Brave</option>
          <option value="file">{tr('ไฟล์ cookies.txt')}</option>
        </select>
      </div>
      {#if ytCookies.startsWith('file:')}
        <div class="tool">
          <div><b>{tr('พาธไฟล์ cookies.txt')}</b></div>
          <input type="text" name="ytcookiefile" autocomplete="off" spellcheck="false" placeholder={tr('เช่น C:\\Users\\คุณ\\cookies.txt…')} value={ytCookies.slice(5)} oninput={(e) => (ytCookies = 'file:' + e.currentTarget.value)} />
        </div>
      {/if}
      {/if}
      {#if toolTab === 'net'}
      <h2>Discord</h2>
      <div class="tool">
        <div><b>{tr('แสดงเพลงที่ฟังอยู่ในสถานะ Discord')}</b><small class="dim">{tr('ไม่ต้องเพิ่มแอปใน Discord เอง แค่เปิด Discord แบบแอปบนคอมไว้ แล้วกด “เชื่อมต่อ / ทดสอบ” (และเปิด Activity Privacy ในการตั้งค่า Discord)')}</small></div>
        <label class="switch"><input type="checkbox" name="discord" bind:checked={discord} /><span>{discord ? tr('เปิด') : tr('ปิด')}</span></label>
        <button class="ghost" onclick={testDiscord}>{tr('เชื่อมต่อ / ทดสอบ')}</button>
      </div>
      <div class="tool">
        <div><b>{tr('แสดงเนื้อเพลงในสถานะ Discord')}</b><small class="dim">{tr('บรรทัดเนื้อเพลงจะขึ้นใต้ชื่อเพลง (ต้องเป็นเพลงที่มีเนื้อแบบมีเวลา) อาจช้ากว่าเสียงเล็กน้อยเพราะ Discord จำกัดความถี่ในการอัปเดต')}</small></div>
        <label class="switch"><input type="checkbox" name="discordly" bind:checked={discordLy} disabled={!discord} /><span>{discordLy ? tr('เปิด') : tr('ปิด')}</span></label>
      </div>
      <p class="dim" role="status" aria-live="polite">{discord ? discordMsg : ''}</p>
      {/if}
      {#if toolTab === 'lib'}
      <h2>{tr('ส่งออกคลังเพลง')}</h2>
      <div class="tool">
        <div><b>{tr('คัดลอกทุกเพลงไปโฟลเดอร์อื่น')}</b><small class="dim">{tr('ตั้งชื่อไฟล์เป็น “ศิลปิน - ชื่อเพลง” ถ้ามี ffmpeg จะฝังชื่อ ศิลปิน อัลบั้ม และปกให้ (ไฟล์ m4a/mp3)')}</small></div>
        <button class="ghost" disabled={toolBusy || !library.length} onclick={exportAll}>{tr('เลือกโฟลเดอร์แล้วส่งออก…')}</button>
      </div>
      <p class="dim" role="status" aria-live="polite">{toolMsg}</p>
      {/if}
      {#if toolTab === 'lib'}
      <h2>{tr('ตรวจไฟล์เพลง')}</h2>
      {#if missingTracks.length}
        <p class="dim">{tr('พบเพลงที่ไฟล์เสียงหายไปจากเครื่อง {0} เพลง', missingTracks.length)}</p>
        <button class="ghost" onclick={() => ask(tr('เอา {0} เพลงที่ไฟล์หายออกจากคลัง?', missingTracks.length), tr('เอาออก'), () => removeTracks(missingTracks))}>{tr('เอาเพลงที่ไฟล์หายออกจากคลัง…')}</button>
      {:else}
        <p class="dim">{tr('ไฟล์เพลงทุกไฟล์อยู่ครบ')}</p>
      {/if}
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
      <h2>{tr('สีเน้น')}</h2>
      <div class="seg wide" role="group" aria-label={tr('แบบสีเน้น')}>
        <button type="button" class:on={look.accentMode === 'solid'} aria-pressed={look.accentMode === 'solid'} onclick={() => (look.accentMode = 'solid')}>{tr('สีเดียว')}</button>
        <button type="button" class:on={look.accentMode === 'cycle'} aria-pressed={look.accentMode === 'cycle'} onclick={() => (look.accentMode = 'cycle')}>{tr('วนหลายสี')}</button>
        <button type="button" class:on={look.accentMode === 'cover'} aria-pressed={look.accentMode === 'cover'} onclick={() => (look.accentMode = 'cover')}>{tr('ตามปกเพลง')}</button>
      </div>
      {#if look.accentMode === 'cover'}
        <p class="dim">{tr('สีเน้นจะเปลี่ยนตามปกเพลงที่กำลังเล่น ถ้าเพลงไม่มีปก จะใช้สีจากโหมดสีเดียว')}</p>
      {:else if look.accentMode === 'solid'}
        {@const c = rgbOf(look.accent)}
        <div class="accbox">
          <input type="color" class="bigsw" name="accent" aria-label={tr('สีเน้น')} bind:value={look.accent} />
          <div class="rgbs">
            {#each ['R', 'G', 'B'] as ch, i (ch)}
              {@const lo = c.map((x, j) => (j === i ? 0 : x))}
              {@const hi = c.map((x, j) => (j === i ? 255 : x))}
              <label class="rgb">
                <span>{ch}</span>
                <input type="range" min="0" max="255" value={c[i]} aria-label={ch} style:--track={`linear-gradient(90deg, rgb(${lo}), rgb(${hi}))`} oninput={(e) => setRgb(i, +e.currentTarget.value)} />
                <input type="number" min="0" max="255" value={c[i]} aria-label={ch} onchange={(e) => setRgb(i, +e.currentTarget.value)} />
              </label>
            {/each}
            <label class="rgb"><span>HEX</span><input type="text" class="hex" maxlength="7" spellcheck="false" autocomplete="off" value={look.accent} onchange={(e) => setHex(e.currentTarget.value)} /></label>
          </div>
        </div>
        <div class="saved">
          <button class="ghost" onclick={saveAccent}>{tr('บันทึกสีนี้')}</button>
          {#each look.accentSaved as h (h)}
            <span class="chip">
              <button type="button" class="dot" class:on={h === look.accent} style:background={h} aria-label={tr('ใช้สี {0}', h)} title={h} onclick={() => (look.accent = h)}></button>
              <button type="button" class="x" aria-label={tr('ลบสี {0}', h)} onclick={() => (look.accentSaved = look.accentSaved.filter((x) => x !== h))}>×</button>
            </span>
          {/each}
        </div>
      {:else}
        <div class="cyprev" style:background={cycleGradient(look.accentStops)} aria-hidden="true"></div>
        <div class="accbox">
          <div class="stepper" role="group" aria-label={tr('จำนวนสี')}>
            <span class="dim">{tr('จำนวนสี')}</span>
            <button type="button" class="ghost" aria-label={tr('ลดจำนวนสี')} disabled={look.accentStops.length <= 2} onclick={() => setStopCount(look.accentStops.length - 1)}>−</button>
            <b>{look.accentStops.length}</b>
            <button type="button" class="ghost" aria-label={tr('เพิ่มจำนวนสี')} disabled={look.accentStops.length >= 8} onclick={() => setStopCount(look.accentStops.length + 1)}>+</button>
          </div>
          <div class="stops">
            {#each look.accentStops as _, i (i)}
              <input type="color" aria-label={tr('สีที่ {0}', i + 1)} bind:value={look.accentStops[i]} />
            {/each}
          </div>
          <button class="ghost rainbow" onclick={() => (look.accentStops = [...RAINBOW])}>{tr('สีรุ้ง')}</button>
        </div>
        <label class="speed">
          <span>{tr('ความเร็ว')} <b>{look.accentSpeed}</b> <small class="dim">({tr('{0} วินาที/รอบ', cycleSeconds(look.accentSpeed).toFixed(cycleSeconds(look.accentSpeed) < 10 ? 1 : 0))})</small></span>
          <input type="range" min="1" max="10" step="1" bind:value={look.accentSpeed} aria-label={tr('ความเร็วการวนสี')} />
          <span class="ends dim"><small>{tr('ช้า')}</small><small>{tr('เร็ว')}</small></span>
        </label>
        <div class="saved">
          <button class="ghost" onclick={saveSet}>{tr('บันทึกชุดสีนี้')}</button>
          {#each look.accentSets as set, i (i)}
            <span class="chip">
              <button type="button" class="setbtn" class:on={sameStops(set.stops, look.accentStops)} style:background={cycleGradient(set.stops)} aria-label={tr('ใช้ชุดสี {0}', set.name)} title={set.name} onclick={() => (look.accentStops = [...set.stops])}></button>
              <button type="button" class="x" aria-label={tr('ลบชุดสี {0}', set.name)} onclick={() => (look.accentSets = look.accentSets.filter((_, j) => j !== i))}>×</button>
            </span>
          {/each}
        </div>
      {/if}
      <h2>{tr('ปรับสีเอง')}</h2>
      <div class="colors">
        <label>{tr('พื้นแผง')}<input type="color" name="base" bind:value={look.base} /></label>
        <label>{tr('พื้นการ์ด')}<input type="color" name="raised" bind:value={look.raised} /></label>
        <label>{tr('พื้นหลังนอกแผง')}<input type="color" name="outer" bind:value={look.outer} /></label>
      </div>
      <p class="dim">{tr('ตัวอักษรและเส้นขอบจะปรับสีเองตามความสว่างของพื้นแผง')}</p>
      <div class="bgpick">
        <button class="ghost" onclick={exportTheme}>{tr('ส่งออกธีม…')}</button>
        <button class="ghost" onclick={importTheme}>{tr('นำเข้าธีม…')}</button>
      </div>
      <h2>{tr('รูปพื้นหลัง')}</h2>
      <div class="bgpick">
        <button class="ghost" disabled={bgBusy} onclick={pickBg}>{tr('เลือกรูป…')}</button>
        <input type="url" name="bgurl" autocomplete="off" spellcheck="false" aria-label={tr('ลิงก์รูปพื้นหลัง')} placeholder={tr('หรือวางลิงก์รูป…')} bind:value={bgUrl} />
        <button class="ghost" disabled={bgBusy || !bgUrl.trim()} onclick={() => setBg(bgUrl)}>{tr('ใช้รูปนี้')}</button>
        <button class="ghost" disabled={bgBusy} onclick={pickBgVideo}>{tr('เลือกวิดีโอ…')}</button>
        {#if look.bg}<button class="ghost" onclick={matchBg}>{tr('ปรับสีตามรูป')}</button>{/if}
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
        <label>{tr('ช่องไฟระหว่างแผง')} <output>{look.gap}px</output><input type="range" name="gap" min="0" max="16" bind:value={look.gap} /></label>
        <label>{tr('ขนาดตัวอักษร')} <output>{look.font}%</output><input type="range" name="font" min="85" max="125" step="5" bind:value={look.font} /></label>
      </div>
      <h2>{tr('ตัวอักษร')}</h2>
      <div class="themes">
        {#each [['system', 'ค่าเริ่มต้นของระบบ', "Inter, 'Segoe UI', 'Leelawadee UI', sans-serif"], ['leelawadee', 'Leelawadee UI', "'Leelawadee UI', sans-serif"], ['sarabun', 'Sarabun', "'Sarabun', sans-serif"], ['noto', 'Noto Sans Thai', "'Noto Sans Thai', sans-serif"]] as [k, l, ff] (k)}
          <button class="theme" class:on={look.fontFace === k} aria-pressed={look.fontFace === k} onclick={() => (look.fontFace = k as typeof look.fontFace)} style:font-family={ff}>
            <b>{tr(l)}</b><small>สวัสดี MySound 123</small>
          </button>
        {/each}
      </div>
      <h2>{tr('อื่นๆ')}</h2>
      <label class="switch"><input type="checkbox" name="hc" bind:checked={look.hc} /><span>{tr('คอนทราสต์สูง (ตัวอักษรชัดขึ้น เส้นขอบเข้มขึ้น)')}</span></label>
      <label class="switch"><input type="checkbox" name="tint" bind:checked={look.tint} /><span>{tr('แผงขวาไล่สีตามปกเพลง')}</span></label>
      <label class="switch"><input type="checkbox" name="motion" bind:checked={look.motion} /><span>{tr('แอนิเมชันของแอป (ปิดแล้วทุกอย่างเปลี่ยนทันที)')}</span></label>
      {#if look.motion}<label class="switch"><input type="checkbox" name="breathe" bind:checked={look.breathe} /><span>{tr('ปกเพลงหายใจเบา ๆ ตอนเล่น')}</span></label>{/if}
      {#if reduced}<p class="dim">{tr('Windows ตั้งให้ลดการเคลื่อนไหวไว้ แอนิเมชันจึงปิดอยู่')}</p>{/if}
      <label class="switch"><input type="checkbox" name="viz" bind:checked={look.viz} /><span>{tr('แสดงคลื่นเสียงในแผง “กำลังเล่น”')}</span></label>
      {#if look.viz}
        <label class="smartq">{tr('รูปแบบคลื่นเสียง')}
          <select name="vizstyle" bind:value={look.vizStyle}><option value="bars">{tr('แท่ง')}</option><option value="wave">{tr('เส้นคลื่น')}</option><option value="circle">{tr('วงกลม')}</option></select>
        </label>
      {/if}
      <label class="switch"><input type="checkbox" name="startPanel" bind:checked={look.startPanel} /><span>{tr('เปิดแผงคิวตอนเริ่มแอป')}</span></label>
      <p><button class="ghost" onclick={() => (look = { ...DEFAULT_LOOK })}>{tr('คืนค่าเริ่มต้นทั้งหมด')}</button></p>
    {:else if pl}
      <div class="plhead">
        {#if pl.image}
          <span class="art big" style:background={plTile(pl)}></span>
        {:else if mosaic(plTracks).length >= 4}
          <span class="art big mosaic" aria-hidden="true">{#each mosaic(plTracks) as t (t.id)}<i style:background={art(t)}></i>{/each}</span>
        {:else}
          <span class="art big" style:background={plTracks.find((t) => t.cover) ? art(plTracks.find((t) => t.cover)!) : cover(pl.name)}></span>
        {/if}
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
          <p class="dim">{tr('{0} เพลง', plTracks.length)}{plTracks.length ? ' · ' + totalDur(plTracks) : ''}</p>
        </div>
      </div>
      <div class="plbar">
        {#if plTracks[0]}<button class="playall" aria-label={tr('เล่นทั้งเพลย์ลิสต์')} onclick={() => play(plTracks[0], plTracks)}><Icon n="play" size={22} /></button>
        <button class="icon big" aria-label={tr('สุ่มเล่นเพลย์ลิสต์')} title={tr('สุ่มเล่น')} onclick={() => shufflePlay(plTracks)}><Icon n="shuffle" size={22} /></button>{/if}
        <button class="ghost" onclick={startRename}>{tr('เปลี่ยนชื่อ')}</button>
        <button class="ghost danger" onclick={askDeletePlaylist}>{tr('ลบเพลย์ลิสต์')}</button>
        <button class="ghost danger" onclick={() => pl && plDeleteAll(pl)}>{tr('ลบเพลย์ลิสต์และเพลงในนั้น')}</button>
      </div>
      <div class="plbar">
        <button class="ghost" onclick={() => pl && exportM3u(pl)}>{tr('ส่งออก M3U…')}</button>
        <button class="ghost" onclick={() => pl && sharePlaylist(pl)}>{tr('แชร์เป็นไฟล์…')}</button>
        <button class="ghost" onclick={() => pl && (pl.pinned = !pl.pinned)}>{pl.pinned ? tr('เลิกปักหมุด') : tr('ปักหมุด')}</button>
        <label class="smartq">{tr('โฟลเดอร์')}<input name="plfolder" list="plfolders" autocomplete="off" placeholder={tr('ไม่มี')} value={pl.folder ?? ''} onchange={(e) => pl && (pl.folder = e.currentTarget.value.trim() || undefined)} /></label>
        <datalist id="plfolders">{#each [...new Set(playlists.map((p) => p.folder).filter(Boolean))] as f (f)}<option value={f}></option>{/each}</datalist>
      </div>
      {@render rows(plTracks, pl.id)}
    {:else if auto}
      <h1>{tr(auto.name)}</h1>
      <p class="dim">{tr('เพลย์ลิสต์อัตโนมัติ คำนวณจากประวัติการฟัง · {0} เพลง', autoTracks.length)}</p>
      {#if autoTracks[0]}<button class="playall" aria-label={tr('เล่นทั้งหมด')} onclick={() => play(autoTracks[0], autoTracks)}><Icon n="play" size={22} /></button>{/if}
      {@render rows(autoTracks)}
    {/if}
  </div>{/key}
  </main>

  <!-- ขวา: คิว / กำลังเล่น / เนื้อเพลง -->
  {#if panel}
    <aside class="right panel">
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div class="resizer r-right" role="separator" aria-orientation="vertical" aria-label={tr('ปรับความกว้างแผงขวา')} aria-valuenow={look.right} aria-valuemin={240} aria-valuemax={440} tabindex="0" onpointerdown={(e) => resizer(e, 'right')} onkeydown={(e) => resizeKey(e, 'right')}></div>
      {#if panel === 'now' && current}
        <div class="art huge" style:background={art(current)}></div>
        <h2>{current.title}</h2>
        <p class="dim">{tr(current.artist)} · {current.album}</p>
        {#if look.viz}<canvas bind:this={vizCanvas} class="viz" width="280" height="64" aria-hidden="true"></canvas>{/if}
        <button class="ghost" onclick={shareCard}>{tr('สร้างการ์ดแชร์ (PNG)…')}</button>
      {:else if panel === 'lyrics'}
        <h2>{tr('เนื้อเพลง')}</h2>
        {#if current}<button class="ghost lybtn" onclick={openLyEdit}>{current.lrc ? tr('แก้เนื้อเพลงที่ตั้งเอง') : tr('ใส่เนื้อเพลงเอง / นำเข้า .lrc')}</button>{/if}
        {#if current}<div class="lytools"><button class="ghost" onclick={openKaraoke}>{tr('เต็มจอ')}</button><button class="ghost" onclick={openLyFloat}>{tr('หน้าต่างลอย')}</button></div>{/if}
        {#if !current}
          <p class="dim">{tr('เล่นเพลงสักเพลงเพื่อดูเนื้อเพลง')}</p>
        {:else if ly.status === 'loading'}
          <div class="skel" role="status" aria-label={tr('กำลังค้นหาเนื้อเพลง…')}>{#each [70, 90, 55, 80, 65, 85] as w, i (i)}<i style:width="{w}%"></i>{/each}</div>
        {:else if ly.status === 'error'}
          <p class="errtxt">{ly.msg}</p>
          <button class="ghost" onclick={() => current && loadLyrics(current, true)}>{tr('ลองใหม่')}</button>
        {:else if ly.status === 'none'}
          <p class="dim">{tr('ไม่พบเนื้อเพลงของเพลงนี้ ลองแก้ชื่อเพลง/ศิลปินให้ตรง (คลิกขวา &gt; ตั้งค่าเพลง) แล้วเปิดแผงนี้ใหม่')}</p>
        {:else if ly.lines.length}
          <div class="lyoff" role="group" aria-label={tr('ปรับเวลาเนื้อเพลง')}>
            <small class="dim">{tr('ปรับเวลา')}</small>
            <button class="ghost" onclick={() => nudgeLy(-0.1)} aria-label={tr('เนื้อเพลงเร็วขึ้น 0.1 วินาที')}>−0.1</button>
            <output>{lyOff > 0 ? '+' : ''}{lyOff.toFixed(1)}s</output>
            <button class="ghost" onclick={() => nudgeLy(0.1)} aria-label={tr('เนื้อเพลงช้าลง 0.1 วินาที')}>+0.1</button>
            {#if lyOff}<button class="ghost" onclick={() => current && (current.lyOff = 0)}>{tr('รีเซ็ต')}</button>{/if}
          </div>
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
        <label class="stopafter">{tr('หยุดเล่นอัตโนมัติ')}
          <select name="stopafter" bind:value={stopAfter}>
            <option value={0}>{tr('ไม่หยุด')}</option>
            <option value={1}>{tr('หลังจบเพลงนี้')}</option>
            {#each [2, 3, 5, 10] as n}<option value={n}>{tr('หลังอีก {0} เพลง', n)}</option>{/each}
          </select>
        </label>
        {#if current}
          <small class="dim">{tr('กำลังเล่น')}</small>
          <div class="qrow now" role="presentation" oncontextmenu={(e) => menu(e, current!)}><span class="thumb" style:background={art(current)}></span><span class="two"><b>{current.title}</b><small>{tr(current.artist)}</small></span></div>
        {/if}
        {#if uq.length}
          <small class="dim">{tr('ในคิวที่เพิ่มเอง')} <button class="link" onclick={() => (uq = [])}>{tr('ล้าง')}</button></small>
          {#each uq as id, i (id + ':' + i)}
            {@const t = library.find((x) => x.id === id)}
            {#if t}
              <div class="qrow" role="presentation">
                <button class="qmain" onclick={() => (uq.splice(i, 1), play(t, undefined, true))}>
                  <span class="thumb" style:background={art(t)}></span>
                  <span class="two"><b>{t.title}</b><small>{tr(t.artist)}</small></span>
                </button>
                <button class="icon" aria-label={tr('เอาออกจากคิว')} onclick={() => uq.splice(i, 1)}>×</button>
              </div>
            {/if}
          {/each}
        {/if}
        <small class="dim">{tr('ถัดไป')}</small>
        {#each queue as t (t.id)}
          <button class="qrow" onclick={() => play(t, srcIds ? srcTracks : undefined)} oncontextmenu={(e) => menu(e, t)}>
            <span class="thumb" style:background={art(t)}></span>
            <span class="two"><b>{t.title}</b><small>{tr(t.artist)}</small></span>
          </button>
        {/each}
      {/if}
    </aside>
  {/if}

  <!-- แถบเล่นเพลง -->
  <footer class="npbar panel">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="np-left" oncontextmenu={(e) => current && menu(e, current)}>
      {#if current}
        <span class="covwrap" class:breathe={playing && look.breathe && moving}>
          {#key current.id}
            <button class="thumb big nfbtn" in:coverIn out:fadeOut style:view-transition-name={nowFull ? 'none' : 'nowcover'} style:background={art(current)} aria-label={tr('เปิดหน้ากำลังเล่นเต็มหน้าต่าง')} onclick={() => setNowFull(true)}></button>
          {/key}
        </span>
        {#key current.id}<span class="two" in:textIn><b use:marquee>{current.title}</b><small>{tr(current.artist)}</small></span>{/key}
        <button class="icon heart" class:on={current.fav} aria-label={current.fav ? tr('เลิกถูกใจเพลงนี้') : tr('ถูกใจเพลงนี้')} aria-pressed={!!current.fav} onclick={(e) => favClick(e, current!)}><Icon n="heart" size={18} fill={!!current.fav} /></button>
      {/if}
    </div>
    <div class="np-mid">
      <div class="ctrl">
        <button class="icon" class:on={shuffle} aria-label={tr('สุ่ม')} onclick={() => (shuffle = !shuffle)}><Icon n="shuffle" /></button>
        {#if current && current.dur > 900}<button class="icon skip" aria-label={tr('ถอยหลัง 15 วินาที')} onclick={() => audio && (audio.currentTime = Math.max(0, audio.currentTime - 15))}>−15</button>{/if}
        <button class="icon" aria-label={tr('ก่อนหน้า')} onclick={prev}><Icon n="prev" /></button>
        <button class="playbtn" class:playing aria-label={playing ? tr('หยุด') : tr('เล่น')} onclick={toggle}><span class="pp"><Icon n="play" size={20} /><Icon n="pause" size={20} /></span></button>
        <button class="icon" aria-label={tr('ถัดไป')} onclick={next}><Icon n="next" /></button>
        <button class="icon skip ab" class:on={abA !== null} aria-label={tr('วนซ้ำช่วงที่เลือก (A–B)')} title={tr('วนซ้ำช่วงที่เลือก: กดครั้งแรกตั้งจุด A ครั้งที่สองตั้งจุด B ครั้งที่สามยกเลิก')} onclick={toggleAB}>{abA === null ? 'A–B' : abB === null ? 'A…' : 'A–B ✕'}</button>
        {#if current && current.dur > 900}<button class="icon skip" aria-label={tr('ข้ามไป 15 วินาที')} onclick={() => audio && (audio.currentTime += 15)}>+15</button>{/if}
        <button class="icon" class:on={repeat} aria-label={tr('เล่นซ้ำ')} onclick={() => (repeat = !repeat)}><Icon n="repeat" /></button>
      </div>
      <div class="seek">
        <small>{fmt(pos)}</small>
        <div class="seekwrap" role="presentation" onpointermove={seekMove} onpointerleave={() => (seekHover = null)}>
          {#if current?.wave}
            <svg class="wave" viewBox="0 0 120 20" preserveAspectRatio="none" aria-hidden="true">
              {#each current.wave as c, i (i)}<rect x={i + 0.15} y={10 - (+c + 1)} width="0.7" height={(+c + 1) * 2} class:played={current.dur && i / 120 < pos / current.dur} />{/each}
            </svg>
          {/if}
          <input type="range" class="trk" style:--p="{current?.dur ? (pos / current.dur) * 100 : 0}%" min="0" max={current?.dur ?? 0} step="1" value={pos} oninput={(e) => audio && (audio.currentTime = +e.currentTarget.value)} aria-label={tr('ตำแหน่งเพลง')} aria-valuetext={`${fmt(pos)} / ${fmt(current?.dur ?? 0)}`} class:haswave={!!current?.wave} />
          {#if seekHover}<span class="seektip" style:left="{seekHover.x}px">{fmt(seekHover.t)}</span><i class="seekline" style:left="{seekHover.x}px" aria-hidden="true"></i>{/if}
        </div>
        <small>{fmt(current?.dur ?? 0)}</small>
      </div>
      {#if barLy && current}<p class="barly" class:on={!!barLine}>{barLine}</p>{/if}
    </div>
    <div class="np-right">
      <button class="icon tab" class:on={panel === 'lyrics'} aria-label={tr('เนื้อเพลง')} title={tr('เนื้อเพลง')} onclick={() => (panel = panel === 'lyrics' ? null : 'lyrics')}><Icon n="lyrics" /><span class="lbl">{tr('เนื้อเพลง')}</span></button>
      <button class="icon tab" class:on={panel === 'queue'} aria-label={tr('คิว')} title={tr('คิว')} onclick={() => (panel = panel === 'queue' ? null : 'queue')}><Icon n="queue" /><span class="lbl">{tr('คิว')}</span></button>
      <button class="icon tab" class:on={panel === 'now'} aria-label={tr('กำลังเล่น')} title={tr('กำลังเล่น')} onclick={() => (panel = panel === 'now' ? null : 'now')}><Icon n="songs" /><span class="lbl">{tr('กำลังเล่น')}</span></button>
      {#if speed !== 1}<button class="icon speed on" aria-label={tr('ความเร็วเล่น {0} เท่า กดเพื่อเปลี่ยน', speed)} title={tr('ความเร็วเล่น')} onclick={cycleSpeed}>{speed}×</button>{/if}
      {#if sleepMin > 0}<button class="icon sleep on" aria-label={tr('ตั้งเวลาปิดเสียง {0} นาที กดเพื่อเปลี่ยน', sleepMin)} title={tr('ตั้งเวลาปิดเสียง')} onclick={cycleSleep}><Icon n="moon" size={18} /><small>{fmt(sleepLeft)}</small></button>{/if}
      <span class="npmorewrap">
        <button class="icon" class:on={npMore} aria-label={tr('ตัวเลือกเพิ่มเติม')} title={tr('ตัวเลือกเพิ่มเติม')} aria-haspopup="menu" aria-expanded={npMore} onclick={(e) => { e.stopPropagation(); const r = e.currentTarget.getBoundingClientRect(); npMoreAt = { right: innerWidth - r.right, bottom: innerHeight - r.top + 8 }; npMore = !npMore; }}><Icon n="more" size={18} /></button>
        {#if npMore}
          <div class="npmore" role="menu" tabindex="-1" style:right="{npMoreAt.right}px" style:bottom="{npMoreAt.bottom}px" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && (npMore = false)}>
            <button role="menuitem" onclick={cycleSpeed}><span>{tr('ความเร็วเล่น')}</span><b>{speed}×</b></button>
            <button role="menuitem" onclick={cycleSleep}><Icon n="moon" size={16} /><span>{tr('ตั้งเวลาปิดเสียง')}</span><b>{sleepMin ? tr('{0} นาที', sleepMin) : tr('ปิดอยู่')}</b></button>
            <button role="menuitem" onclick={() => { npMore = false; openMini(); }}><Icon n="mini" size={16} /><span>{tr('มินิเพลเยอร์')}</span></button>
          </div>
        {/if}
      </span>
      <button class="icon" aria-label={volume ? tr('ปิดเสียง') : tr('เปิดเสียง')} aria-pressed={volume === 0} onclick={toggleMute} onwheel={wheelVol}><Icon n="volume" size={18} /></button>
      <input class="vol trk" style:--p="{volume}%" type="range" min="0" max="100" bind:value={volume} onwheel={wheelVol} aria-label={tr('ระดับเสียง')} />
      <small class="volv" aria-hidden="true">{volume}%</small>
    </div>
  </footer>
</div>

{#if drag}
  <div class="dragghost" style:left="{drag.x + 14}px" style:top="{drag.y + 10}px" aria-hidden="true">{drag.label}</div>
{/if}

{#if nowFull && current}
  <div class="nowfull" role="dialog" aria-modal="true" aria-label={tr('กำลังเล่น')}>
    <div class="kbg" style:background={art(current)}></div>
    <button class="kclose ghost" onclick={() => setNowFull(false)}>{tr('ปิด')} (Esc)</button>
    <div class="nfcover" class:breathe={playing && look.breathe && moving} style:background={art(current)}></div>
    <div class="nfmeta">
      <h1>{current.title}</h1>
      <p><button class="alink" onclick={() => { setNowFull(false); view = 'artist:' + current!.artist; }}>{tr(current.artist)}</button>{current.album ? ' · ' + current.album : ''}</p>
      {#if curLine}<p class="nfly">{curLine}</p>{/if}
      <div class="seek"><small>{fmt(pos)}</small><input type="range" min="0" max={current.dur} step="1" value={pos} oninput={(e) => audio && (audio.currentTime = +e.currentTarget.value)} aria-label={tr('ตำแหน่งเพลง')} /><small>{fmt(current.dur)}</small></div>
      <div class="ctrl nfctrl">
        <button class="icon" class:on={shuffle} aria-label={tr('สุ่ม')} onclick={() => (shuffle = !shuffle)}><Icon n="shuffle" /></button>
        <button class="icon" aria-label={tr('ก่อนหน้า')} onclick={prev}><Icon n="prev" /></button>
        <button class="playbtn big" class:playing aria-label={playing ? tr('หยุด') : tr('เล่น')} onclick={toggle}><span class="pp"><Icon n="play" size={28} /><Icon n="pause" size={28} /></span></button>
        <button class="icon" aria-label={tr('ถัดไป')} onclick={() => next()}><Icon n="next" /></button>
        <button class="icon" class:on={repeat} aria-label={tr('เล่นซ้ำ')} onclick={() => (repeat = !repeat)}><Icon n="repeat" /></button>
      </div>
      {#if queue.length || uq.length}
        <small class="dim">{tr('ถัดไป')}: {(library.find((x) => x.id === uq[0]) ?? queue[0])?.title ?? ''}</small>
      {/if}
    </div>
  </div>
{/if}

{#if dragOver}
  <div class="dropov" aria-hidden="true"><div>{tr('วางไฟล์เพลง หรือไฟล์เพลย์ลิสต์ (.m3u, .mysound, .csv) เพื่อนำเข้า')}</div></div>
{/if}

{#if karaoke && current}
  <div class="karaoke" role="dialog" aria-modal="true" aria-label={tr('เนื้อเพลงเต็มจอ')}>
    <div class="kbg" style:background={art(current)}></div>
    <button class="kclose ghost" onclick={closeKaraoke}>{tr('ปิด')} (Esc)</button>
    <div class="kmeta"><b>{current.title}</b> · {tr(current.artist)}</div>
    {#if ly.status === 'ready' && ly.lines.length}
      <p class="kl kprev">{kIdx > 0 ? ly.lines[kIdx - 1].text : ''}</p>
      <p class="kl kcur" style:--p={kProg}>{kIdx >= 0 ? ly.lines[kIdx].text || '♪' : '♪'}</p>
      <p class="kl knext">{ly.lines[kIdx + 1]?.text ?? ''}</p>
      <p class="kl knext2">{ly.lines[kIdx + 2]?.text ?? ''}</p>
    {:else if ly.status === 'ready'}
      <p class="kplain">{ly.plain}</p>
    {:else}
      <p class="kl kcur">{ly.status === 'loading' ? tr('กำลังค้นหาเนื้อเพลง…') : tr('ไม่พบเนื้อเพลงของเพลงนี้')}</p>
    {/if}
  </div>
{/if}

{#if flash}
  <div in:toastT out:toastT={{ inn: false }} class="toast n flash" role="status" onpointerenter={() => (flashHold = true)} onpointerleave={() => (flashHold = false)}><span>{flash}</span>{#if flashAct}<button class="ghost sm" onclick={flashDo}>{flashAct.label}</button>{/if}</div>
{/if}

{#if helpOpen}
  <div class="helpbg" role="presentation" onclick={() => (helpOpen = false)}>
    <div class="helpbox" role="dialog" aria-modal="true" aria-label={tr('ปุ่มลัด')} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === 'Escape' && (helpOpen = false)}>
      <h2>{tr('ปุ่มลัด')}</h2>
      {#each KEYS as g (g.title)}
        <h3>{tr(g.title)}</h3>
        <dl>
          {#each g.rows as [k, d] (k)}
            <dt><kbd>{k}</kbd></dt><dd>{tr(d)}</dd>
          {/each}
        </dl>
      {/each}
      <button class="ghost" onclick={() => (helpOpen = false)}>{tr('ปิด')}</button>
    </div>
  </div>
{/if}

{#if upd && !updDismissed}
  <div in:toastT out:toastT={{ inn: false }} class="toast n" role="status">
    <span>{tr('มีเวอร์ชันใหม่ {0}', upd.version)}</span>
    <button class="ghost" disabled={updBusy} onclick={() => installUpdate()}>{updBusy ? tr('กำลังโหลด…') : tr('อัปเดตเลย')}</button>
    <button class="ghost" onclick={() => (updDismissed = true)}>{tr('ไว้ก่อน')}</button>
  </div>
{/if}

{#if notice}
  <div in:toastT out:toastT={{ inn: false }} class="toast n" role="status">
    <span class="errtxt">{notice}</span>
    {#if badTrack}<button class="ghost" onclick={() => { const t = badTrack; badTrack = null; notice = ''; if (t) toTrash([t]); }}>{tr('ลบเพลงนี้')}</button>{/if}
    <button class="ghost" onclick={() => { notice = ''; badTrack = null; }}>{tr('ปิด')}</button>
  </div>
{/if}

{#if pasted}
  <div in:toastT out:toastT={{ inn: false }} class="toast" role="status">
    <span>{tr('พบลิงก์ในคลิปบอร์ด:')} <b>{pasted.length > 48 ? pasted.slice(0, 48) + '…' : pasted}</b></span>
    <button class="primary sm" onclick={() => addDownload(pasted)}>{tr('โหลดเลย')}</button>
    <button class="ghost" onclick={() => (pasted = '')}>{tr('ปิด')}</button>
  </div>
{/if}

{#if plCtx}
  <div class="ctx" role="menu" tabindex="-1" aria-label={plCtx.p.name} style:left="{plCtx.x}px" style:top="{plCtx.y}px" use:ctxMount onkeydown={ctxKey}>
    <button role="menuitem" onclick={() => { const p = plCtx!.p; plCtx = null; play(byIds(p.ids)[0], byIds(p.ids)); }} disabled={!plCtx.p.ids.length}><Icon n="play" size={16} /><span>{tr('เล่น')}</span></button>
    <button role="menuitem" onclick={() => { const p = plCtx!.p; plCtx = null; shufflePlay(byIds(p.ids)); }} disabled={!plCtx.p.ids.length}><Icon n="shuffle" size={16} /><span>{tr('สุ่มเล่น')}</span></button>
    <hr />
    <button role="menuitem" onclick={() => plRename(plCtx!.p)}><Icon n="tools" size={16} /><span>{tr('เปลี่ยนชื่อ')}</span></button>
    <button role="menuitem" onclick={() => plPickImage(plCtx!.p)}><Icon n="palette" size={16} /><span>{plCtx.p.image ? tr('เปลี่ยนรูป…') : tr('ใส่รูป…')}</span></button>
    {#if plCtx.p.image}<button role="menuitem" onclick={() => plRemoveImage(plCtx!.p)}><Icon n="wave" size={16} /><span>{tr('เอารูปออก (กลับไปใช้สีไล่ระดับ)')}</span></button>{/if}
    <button role="menuitem" onclick={() => { plCtx!.p.pinned = !plCtx!.p.pinned; plCtx = null; }}><Icon n="plus" size={16} /><span>{plCtx.p.pinned ? tr('เลิกปักหมุด') : tr('ปักหมุด')}</span></button>
    <hr />
    <button role="menuitem" class="danger" onclick={() => plDelete(plCtx!.p)}><Icon n="trash" size={16} /><span>{tr('ลบเพลย์ลิสต์')}</span></button>
    <button role="menuitem" class="danger" onclick={() => plDeleteAll(plCtx!.p)}><Icon n="trash" size={16} /><span>{tr('ลบเพลย์ลิสต์และเพลงในนั้น')}</span></button>
  </div>
{/if}

{#if ctx}
  <div class="ctx" role="menu" tabindex="-1" aria-label={ctx.t.title} style:left="{ctx.x}px" style:top="{ctx.y}px" use:ctxMount onkeydown={ctxKey}>
    {#snippet mi(icon: string, label: string, run: () => void, kbd = '', cls = '')}
      <button role="menuitem" class={cls} onclick={act(run)}><Icon n={icon} size={16} /><span>{label}</span>{#if kbd}<kbd>{kbd}</kbd>{/if}</button>
    {/snippet}
    {@render mi('play', tr('เล่น'), () => play(ctx!.t), 'Enter')}
    {@render mi('next', tr('เล่นถัดไป'), () => queueNext(ctx!.t))}
    {@render mi('queue', tr('เพิ่มเข้าคิว'), () => queueAdd(ctx!.t))}
    <hr />
    {@render mi('heart', ctx.t.fav ? tr('เลิกถูกใจ') : tr('ถูกใจ'), () => toggleFav(ctx!.t), 'L')}
    <div class="subwrap" role="none" onmouseenter={() => (subOpen = true)} onmouseleave={() => (subOpen = false)}>
      <button role="menuitem" data-sub="1" aria-haspopup="menu" aria-expanded={subOpen} onclick={() => (subOpen = !subOpen)}><Icon n="plus" size={16} /><span>{tr('เพิ่มลงเพลย์ลิสต์')}</span><kbd>›</kbd></button>
      {#if subOpen}
        <div class="ctx sub" role="menu" aria-label={tr('เพิ่มลงเพลย์ลิสต์')}>
          <button role="menuitem" onclick={act(() => { const id = newPlaylist(); addToPlaylist(id, ctx!.t.id); say(tr('สร้างเพลย์ลิสต์ใหม่และเพิ่มเพลงแล้ว')); })}><Icon n="plus" size={16} /><span>{tr('เพลย์ลิสต์ใหม่')}</span></button>
          {#each playlists.filter((p) => p.id !== ctx!.plId) as p (p.id)}
            <button role="menuitem" disabled={p.ids.includes(ctx.t.id)} onclick={act(() => addToPlaylist(p.id, ctx!.t.id))}><span class="sw" style:background={cover(p.name)}></span><span>{p.name}</span>{#if p.ids.includes(ctx.t.id)}<kbd>✓</kbd>{/if}</button>
          {/each}
        </div>
      {/if}
    </div>
    {#if ctx.plId}
      {@render mi('prev', tr('เลื่อนขึ้น'), () => move(ctx!.plId!, ctx!.t.id, -1))}
      {@render mi('next', tr('เลื่อนลง'), () => move(ctx!.plId!, ctx!.t.id, 1))}
      {@render mi('trash', tr('เอาออกจากเพลย์ลิสต์'), () => { const p = playlists.find((x) => x.id === ctx!.plId); if (p) { checkpoint(); p.ids.splice(p.ids.indexOf(ctx!.t.id), 1); } })}
    {/if}
    <hr />
    {@render mi('artist', tr('ไปที่ศิลปิน'), () => (view = 'artist:' + ctx!.t.artist))}
    {@render mi('download', tr('แปลงเป็น MP3…'), () => toMp3([ctx!.t]))}
    <button role="menuitem" onclick={() => openEdit(ctx!.t)}><Icon n="tools" size={16} /><span>{tr('ตั้งค่าเพลง…')}</span></button>
    <hr />
    {@render mi('trash', tr('ย้ายไปถังขยะ'), () => askDeleteTrack(ctx!.t), 'Del', 'danger')}
  </div>
{/if}

{#if shareOpen && current}
  <ShareCard title={current.title} artist={tr(current.artist)} album={current.album ?? ''} cover={current.cover ? convertFileSrc(current.cover) : null} lines={shareOpen.lines} first={shareOpen.first} accent={themeVars['--accent'] ?? '#5eead4'} onclose={() => (shareOpen = null)} onmsg={say} />
{/if}

<dialog bind:this={bulkDlg} class="edit" aria-labelledby="bulk-h">
  <form onsubmit={(e) => (e.preventDefault(), saveBulk())}>
    <h2 id="bulk-h">{tr('แก้ข้อมูล {0} เพลง', selTracks.length)}</h2>
    <p class="dim">{tr('ช่องที่เว้นว่างจะไม่เปลี่ยน')}</p>
    <div class="fields">
      <label>{tr('ศิลปิน')}<input name="bulkartist" autocomplete="off" bind:value={bed.artist} /></label>
      <label>{tr('อัลบั้ม')}<input name="bulkalbum" autocomplete="off" bind:value={bed.album} /></label>
      <label>{tr('เพิ่มแท็ก (คั่นด้วยจุลภาค)')}<input name="bulkaddtags" autocomplete="off" bind:value={bed.addTags} /></label>
      <label>{tr('เอาแท็กออก')}<input name="bulkdeltags" autocomplete="off" bind:value={bed.delTags} /></label>
    </div>
    <div class="dlgbtns">
      <button type="button" class="ghost" onclick={() => bulkDlg?.close()}>{tr('ยกเลิก')}</button>
      <button type="submit" class="ghost">{tr('บันทึก')}</button>
    </div>
  </form>
</dialog>

<dialog bind:this={dlg} class="edit" aria-labelledby="edit-h">
  <form onsubmit={(e) => (e.preventDefault(), saveEdit())}>
    <h2 id="edit-h">{tr('ตั้งค่าเพลง')}</h2>
    <div class="editrow">
      <span class="art big" style:background={editing ? art(editing) : ''}></span>
      <div class="fields">
        <label>{tr('ชื่อเพลง')}<input name="title" autocomplete="off" bind:value={edit.title} required /></label>
        <label>{tr('ศิลปิน')}<input name="artist" autocomplete="off" bind:value={edit.artist} /></label>
        <label>{tr('อัลบั้ม')}<input name="album" autocomplete="off" bind:value={edit.album} /></label>
        <label>{tr('แท็ก (คั่นด้วยจุลภาค)')}<input name="tags" autocomplete="off" placeholder={tr('เช่น chill, ตอนทำงาน…')} bind:value={edit.tags} /></label>
        <label>{tr('โน้ต')}<textarea name="note" rows="2" bind:value={edit.note}></textarea></label>
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
    <label class="gainrow">{tr('ปรับความดังเพลงนี้')} <output>{(editing?.gainDb ?? 0) > 0 ? '+' : ''}{editing?.gainDb ?? 0} dB</output>
      <input type="range" name="gaindb" min="-12" max="6" step="1" value={editing?.gainDb ?? 0} oninput={(e) => editing && (editing.gainDb = +e.currentTarget.value)} />
    </label>
    <div class="trimrow">
      <label>{tr('ข้ามต้นเพลง (วินาที)')}<input type="number" name="skipstart" min="0" step="0.1" value={editing?.skipStart ?? 0} oninput={(e) => editing && (editing.skipStart = Math.max(0, +e.currentTarget.value || 0))} /></label>
      <label>{tr('ตัดท้ายเพลง (วินาที)')}<input type="number" name="skipend" min="0" step="0.1" value={editing?.skipEnd ?? 0} oninput={(e) => editing && (editing.skipEnd = Math.max(0, +e.currentTarget.value || 0))} /></label>
      <button type="button" class="ghost" disabled={trimBusy} onclick={detectTrim}>{tr('ตรวจความเงียบ')}</button>
    </div>
    <small class="dim" role="status">{trimMsg}</small>
    <h3>{tr('ตัดเพลง')}</h3>
    <div class="trimrow">
      <label>{tr('เริ่ม (วินาที)')}<input type="number" name="clipa" min="0" step="0.1" bind:value={clipA} /></label>
      <label>{tr('จบ (วินาที)')}<input type="number" name="clipb" min="0" step="0.1" bind:value={clipB} /></label>
      {#if current?.id === editing?.id && abA !== null && abB !== null}<button type="button" class="ghost" onclick={() => ((clipA = Math.round(abA! * 10) / 10), (clipB = Math.round(abB! * 10) / 10))}>{tr('ใช้ช่วง A–B')}</button>{/if}
      <button type="button" class="ghost" disabled={cutBusy} onclick={saveClip}>{tr('บันทึกช่วงนี้เป็นเพลงใหม่')}</button>
      {#if editing?.url && /youtu/.test(editing.url)}<button type="button" class="ghost" disabled={cutBusy} onclick={splitChapters}>{tr('แยกตามบทของ YouTube')}</button>{/if}
    </div>
    <small class="dim" role="status">{cutMsg}</small>
    <h3>{tr('เสียง')}</h3>
    <button type="button" class="ghost" disabled={busy} onclick={cleanAudio}>{tr('ตัดความเงียบหัวท้าย + ปรับความดังให้เท่ากัน (ต้องมี ffmpeg)')}</button>
    <p class:errtxt={!editOk} class:oktxt={editOk} role="status">{editMsg}</p>
    <div class="actions">
      <button type="button" class="ghost" onclick={() => dlg?.close()}>{tr('ปิด')}</button>
      <button class="primary">{tr('บันทึก')}</button>
    </div>
  </form>
</dialog>

<dialog bind:this={lyDlg} class="edit lydlg" aria-labelledby="ly-h" onkeydown={syncKey} onclose={() => (sync = null)}>
  <form onsubmit={(e) => (e.preventDefault(), saveLy())}>
    <h2 id="ly-h">{tr('เนื้อเพลงของเพลงนี้')}</h2>
    <p class="dim">{tr('วางเนื้อเพลงลงช่อง ถ้าอยากให้เลื่อนตามเวลา ใช้รูปแบบ LRC เช่น')} <code>{tr('[01:23.50] ข้อความ')}</code> {tr('ต่อบรรทัด หรือเลือกไฟล์ .lrc/.txt')}</p>
    {#if sync}
      <div class="syncbox" role="list" aria-label={tr('ตั้งเวลาเนื้อเพลง')}>
        {#each sync as l, i (i)}
          <div role="listitem" class="syncl" class:on={i === syncI} class:done={l.t !== null}><small>{l.t !== null ? lrcTime(l.t) : '--:--.--'}</small> {l.text}</div>
        {/each}
      </div>
      <p class="dim">{tr('กด Space หรือ Enter ตอนที่บรรทัดที่ไฮไลต์เริ่มร้อง · Backspace ถอยหนึ่งบรรทัด')} ({syncI}/{sync.length})</p>
      <div class="actions">
        <button type="button" class="primary syncbtn" onclick={stamp}>{tr('แตะ: บรรทัดนี้เริ่มแล้ว')}</button>
        <button type="button" class="ghost" onclick={unstamp} disabled={!syncI}>{tr('ถอยหนึ่งบรรทัด')}</button>
        <button type="button" class="ghost" onclick={finishSync}>{tr('เสร็จ')}</button>
        <button type="button" class="ghost" onclick={() => (sync = null)}>{tr('ยกเลิก')}</button>
      </div>
    {:else}
      <textarea name="lyrics" class="lytext" spellcheck="false" aria-label={tr('เนื้อเพลง')} bind:value={lyText} onpaste={(e) => { e.stopPropagation(); setTimeout(() => (lyText = cleanLyrics(lyText)), 0); }}></textarea>
    {/if}
    {#if ai}<div class="aibar" role="progressbar" aria-label={aiText} aria-valuenow={ai.pct} aria-valuemin={0} aria-valuemax={100}><i style:width="{ai.pct}%"></i></div>{/if}
    <p class="errtxt" role="status">{lyMsg}</p>
    <div class="lytools">
      <button type="button" class="ghost" onclick={pickLrc}>{tr('เลือกไฟล์…')}</button>
      {#if !sync}<button type="button" class="ghost" disabled={!!ai} onclick={startSync}>{tr('ตั้งเวลาเอง (แตะตามเพลง)')}</button>{/if}
      {#if !sync}
        {#if ai}
          <button type="button" class="ghost" onclick={aiCancel}>{tr('ยกเลิก')} · {aiText}</button>
        {:else}
          <button type="button" class="ghost" onclick={aiSync}>{tr('ให้ AI จับเวลาให้')}</button>
        {/if}
      {/if}
    </div>
    <div class="actions lyfoot">
      {#if current?.lrc}<button type="button" class="ghost danger" onclick={() => saveLy(true)}>{tr('ลบที่ตั้งเอง (กลับไปใช้ LRCLIB)')}</button>{/if}
      <span class="grow"></span>
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
  .app.withly { --npbar-h: 112px; }
  .aibar { height: 6px; border-radius: 3px; background: var(--bg-raised); overflow: hidden; margin: 8px 0; }
  .aibar i { display: block; height: 100%; background: var(--accent); transition: width 300ms linear; }
  .logomark { display: grid; place-items: center; width: 34px; height: 34px; flex: none; border-radius: 8px; background: #0a0a0a; color: #fff; box-shadow: inset 0 0 0 1px rgba(255,255,255,.08); }
  .logomark :global(svg) { stroke-width: 2.7; }
  /* เปลี่ยนหน้าแบบเฟด + เลื่อนขึ้นเล็กน้อย */
  .vf { animation: vf 180ms var(--ease-out); }
  @keyframes vf { from { opacity: 0; transform: translateY(4px); } }
  .ghost, .seg button, .theme, .ctx button { transition: transform 140ms var(--ease-out), background-color 140ms ease; }
  .ghost:active:not(:disabled), .seg button:active, .theme:active { transform: scale(0.97); }
  .ctx { min-width: 220px; animation: pop 140ms var(--ease-out); }
  .ctx [role='menuitem'] { display: flex; align-items: center; gap: 10px; }
  .ctx [role='menuitem'] span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ctx button:disabled { opacity: 0.5; }
  .ctx kbd { margin-left: 16px; font-size: 0.75rem; color: var(--text-dim); border: 0; padding: 0; }
  .ctx hr { border: 0; border-top: 1px solid var(--line2); margin: 4px 6px; }
  .ctx [role='menuitem'] span.sw { flex: none; width: 16px; height: 16px; border-radius: 3px; }
  .subwrap { position: relative; }
  .ctx.sub { position: absolute; left: calc(100% - 4px); top: -4px; max-height: 320px; overflow: auto; transform-origin: top left; }
  select { height: 32px; padding: 0 8px; border-radius: 6px; border: 1px solid var(--line2); background-color: var(--bg-raised); color: var(--text); font: inherit; }
  select:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .seg { display: flex; border-radius: 6px; overflow: hidden; background: var(--bg-raised); }
  .seg button { display: grid; place-items: center; width: 32px; height: 28px; color: var(--text-dim); }
  .seg button.on { color: var(--text); background: var(--line2); }
  .gridlist { margin-top: 12px; }
  .row.dropbefore { box-shadow: inset 0 2px 0 var(--accent); }
  .row.dropafter { box-shadow: inset 0 -2px 0 var(--accent); }
  .item.dropon { outline: 2px solid var(--accent); outline-offset: -2px; background: var(--bg-raised); }
  .dragghost { animation: lift 180ms var(--ease-out); }
  @keyframes lift { from { opacity: 0; transform: scale(0.9) translateY(4px); } }
  .dragghost { position: fixed; z-index: 80; pointer-events: none; padding: 6px 12px; border-radius: 999px; background: var(--accent); color: var(--on-accent); font-weight: 700; font-size: 0.85rem; box-shadow: 0 6px 18px rgba(0,0,0,.35); max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  :global(body.dragging), :global(body.dragging *) { cursor: grabbing !important; user-select: none; }
  :global(body.resizing), :global(body.resizing *) { cursor: col-resize !important; user-select: none; }
  .side, .right { position: relative; }
  .resizer { position: absolute; top: 0; bottom: 0; width: 8px; z-index: 4; cursor: col-resize; border-radius: 4px; }
  .r-side { right: 0; }
  .r-right { left: 0; }
  .resizer:hover, .resizer:focus-visible { background: color-mix(in srgb, var(--accent) 45%, transparent); outline: 0; }
  .seekwrap { position: relative; flex: 1; display: flex; align-items: center; height: 24px; }
  .seekwrap input { position: relative; z-index: 1; }
  /* มีรูปคลื่น: ซ่อนเส้นของแถบเลื่อน เหลือแค่เส้นบอกตำแหน่ง (ยังคลิก/ลากกรอได้เหมือนเดิม) */
  .seekwrap input.haswave { -webkit-appearance: none; appearance: none; height: 24px; margin: 0; background: transparent; cursor: pointer; }
  .seekwrap input.haswave::-webkit-slider-runnable-track { height: 24px; background: transparent; border: 0; }
  .seekwrap input.haswave::-webkit-slider-thumb { -webkit-appearance: none; width: 3px; height: 24px; border-radius: 2px; background: var(--text); box-shadow: 0 0 0 1px var(--bg-base); }
  .seekwrap input.haswave:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 4px; }
  .wave { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
  .wave rect { fill: var(--text-dim); opacity: 0.45; }
  .wave rect.played { fill: var(--accent); opacity: 1; }
  .seektip { position: absolute; bottom: 24px; transform: translateX(-50%); z-index: 3; padding: 2px 6px; border-radius: 4px; background: var(--bg-hover); font-size: 0.75rem; font-variant-numeric: tabular-nums; pointer-events: none; white-space: nowrap; }
  .nfbtn { padding: 0; cursor: zoom-in; }
  .nowfull { position: fixed; inset: 0; z-index: 55; display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); align-items: center; gap: 6vw; padding: 8vh 7vw; color: #fff; overflow: hidden; background: #000; }
  .nfcover { aspect-ratio: 1; width: min(100%, 64vh); justify-self: end; border-radius: 16px; box-shadow: 0 30px 80px rgba(0,0,0,.55); background-size: cover !important; view-transition-name: nowcover; }
  .nfmeta { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .nfmeta h1 { font-size: clamp(1.8rem, 4vw, 3.2rem); line-height: 1.1; overflow-wrap: anywhere; }
  .nfmeta p { color: rgba(255,255,255,.75); font-size: 1.1rem; }
  .nfmeta .nfly { color: var(--accent); font-weight: 700; font-size: 1.3rem; }
  .nfmeta .seek { max-width: 520px; }
  .nfmeta .icon { color: rgba(255,255,255,.75); }
  .nfctrl { justify-content: flex-start; }
  .playbtn.big { width: 64px; height: 64px; background: #fff; color: #000; }
  @media (max-width: 900px) { .nowfull { grid-template-columns: 1fr; text-align: center; } .nfcover { justify-self: center; width: min(70vw, 40vh); } .nfctrl { justify-content: center; } }
  .mosaic { display: grid !important; grid-template-columns: 1fr 1fr; overflow: hidden; }
  .mosaic i { display: block; background-size: cover !important; }
  .icon.big { width: 48px; height: 48px; }
  .skel { display: flex; flex-direction: column; gap: 10px; margin-top: 8px; }
  .skel i { display: block; height: 14px; border-radius: 6px; background: linear-gradient(90deg, var(--bg-raised) 25%, var(--bg-hover) 50%, var(--bg-raised) 75%); background-size: 200% 100%; animation: shimmer 1.2s linear infinite; }
  .skel.rows i { height: 52px; }
  @keyframes shimmer { to { background-position: -200% 0; } }
  .bgvid { width: 100%; height: 100%; object-fit: cover; }
  .tool.sub, .switch.sub { margin-left: 24px; }
  .tool input[type='url'], .tool input[name^='lfm'] { min-width: 180px; height: 34px; padding: 0 10px; border-radius: 6px; border: 1px solid var(--line2, var(--line)); background: var(--bg-raised); color: var(--text); }
  .remoteurl { display: flex; flex-direction: column; gap: 4px; margin: 4px 0 4px 24px; }
  .remoteurl code { user-select: all; font-size: 1rem; }
  .dropov { position: fixed; inset: 0; z-index: 70; display: grid; place-items: center; background: rgba(0,0,0,.55); pointer-events: none; }
  .dropov div { padding: 28px 36px; border: 2px dashed var(--accent); border-radius: 16px; color: #fff; font-size: 1.1rem; font-weight: 700; background: rgba(0,0,0,.4); max-width: 520px; text-align: center; }
  .lytools { display: flex; gap: 6px; margin: 6px 0; }
  .karaoke { position: fixed; inset: 0; z-index: 60; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 18px; padding: 6vh 8vw; text-align: center; color: #fff; background: #000; overflow: hidden; }
  .kbg { position: absolute; inset: -40px; filter: blur(40px) brightness(0.45); background-size: cover !important; background-position: center !important; z-index: -1; }
  .kclose { position: absolute; top: 16px; right: 16px; color: #fff; }
  .kmeta { position: absolute; top: 20px; left: 24px; opacity: 0.8; }
  .kl { margin: 0; max-width: 100%; font-weight: 800; line-height: 1.25; text-shadow: 0 2px 12px rgba(0,0,0,.6); }
  .kprev { font-size: clamp(1rem, 2.4vw, 1.6rem); opacity: 0.4; }
  .kcur { font-size: clamp(1.8rem, 5vw, 3.6rem); background: linear-gradient(90deg, var(--accent) calc(var(--p, 0) * 100%), #fff calc(var(--p, 0) * 100%)); -webkit-background-clip: text; background-clip: text; color: transparent; text-shadow: none; filter: drop-shadow(0 2px 8px rgba(0,0,0,.6)); }
  .knext { font-size: clamp(1.1rem, 2.8vw, 1.9rem); opacity: 0.7; }
  .knext2 { font-size: clamp(1rem, 2.4vw, 1.6rem); opacity: 0.4; }
  .kplain { white-space: pre-wrap; max-height: 80vh; overflow: auto; font-size: 1.3rem; }
  .syncbox { max-height: 300px; overflow: auto; border: 1px solid var(--line); border-radius: 8px; padding: 6px; }
  .syncl { padding: 3px 6px; border-radius: 4px; color: var(--text-dim); }
  .syncl small { font-variant-numeric: tabular-nums; opacity: 0.7; margin-right: 6px; }
  .syncl.done { color: var(--text); }
  .syncl.on { background: var(--accent); color: #000; font-weight: 700; }
  .bulkbar { position: sticky; top: 0; z-index: 5; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 8px 12px; margin: -8px -8px 12px; border-radius: 8px; background: var(--bg-raised); box-shadow: 0 4px 16px rgba(0,0,0,.25); }
  .bulkbar b { margin-right: 4px; }
  .listtools { display: flex; align-items: center; gap: 12px; justify-content: flex-end; margin-top: 8px; }
  .colmenu { position: relative; font-size: 0.85rem; color: var(--text-dim); }
  .colmenu summary { cursor: pointer; list-style: none; padding: 4px 8px; border-radius: 6px; }
  .colmenu summary:hover { color: var(--text); background: var(--bg-raised); }
  .colpop { position: absolute; right: 0; top: 100%; z-index: 6; display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; border-radius: 8px; background: var(--bg-raised); white-space: nowrap; box-shadow: 0 6px 20px rgba(0,0,0,.3); }
  .sorth { text-align: left; color: inherit; font: inherit; padding: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sorth:hover, .sorth.on { color: var(--text); }
  .sortc { min-width: 0; overflow: hidden; }
  .sortc.r { text-align: right; }
  .alink { color: inherit; font: inherit; padding: 0; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .alink:hover { color: var(--text); text-decoration: underline; }
  .tags { color: var(--accent); }
  .stars-s { color: var(--accent); letter-spacing: -1px; }
  .qhelp { margin: 8px 0; font-size: 0.85rem; color: var(--text-dim); }
  .qhelp code { white-space: normal; }
  .smartq { display: flex; align-items: center; gap: 6px; color: var(--text-dim); }
  .smartq input { min-width: 180px; }
  .trow { display: flex; align-items: center; gap: 12px; }
  .trow .two { flex: 1; min-width: 0; }
  .dupg { display: flex; flex-direction: column; gap: 2px; padding: 8px; margin: 8px 0; border-radius: 8px; border: 1px solid var(--line); max-width: 760px; }
  .dupg .link { align-self: flex-start; margin: 4px 8px; }
  .storedl { display: grid; grid-template-columns: max-content max-content; gap: 6px 24px; margin: 12px 0; font-variant-numeric: tabular-nums; }
  .storedl dd { margin: 0; text-align: right; }
  .plfolder summary { cursor: pointer; padding: 6px 8px; color: var(--text-dim); font-size: 0.85rem; }
  .art.round { border-radius: 50%; }
  .dlgbtns { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }
  .eqbands { display: flex; gap: 10px; justify-content: space-between; max-width: 460px; margin: 4px 0 12px; }
  .eqbands label { display: flex; flex-direction: column; align-items: center; gap: 4px; }
  .eqbands input[type='range'] { writing-mode: vertical-lr; direction: rtl; height: 90px; width: 22px; }
  .trimrow { display: flex; gap: 10px; align-items: end; flex-wrap: wrap; margin: 8px 0 2px; }
  .trimrow input { width: 90px; }
  .ab.on { color: var(--accent); }
  .lyoff { display: flex; align-items: center; gap: 6px; margin-bottom: 8px; }
  .lyoff output { min-width: 4.2em; text-align: center; font-variant-numeric: tabular-nums; }
  .stopafter { display: flex; align-items: center; gap: 8px; margin: 0 0 8px; color: var(--text-dim); font-size: 0.85rem; }
  .qrow:has(.qmain) { display: flex; align-items: center; }
  .qmain { display: flex; align-items: center; gap: 10px; flex: 1; min-width: 0; text-align: left; }
  .skip { font-size: 0.75rem; font-weight: 700; width: auto; padding: 0 6px; }
  .volv { min-width: 3ch; text-align: right; font-variant-numeric: tabular-nums; }
  .gainrow { display: flex; align-items: center; gap: 10px; margin: 8px 0; }
  .gainrow input { flex: 1; }
  .hist { display: flex; flex-direction: column; gap: 2px; max-width: 760px; }
  .hist .qrow { display: flex; align-items: center; gap: 12px; }
  .hist .qrow .dim { margin-left: auto; white-space: nowrap; }
  .toast.flash { bottom: calc(var(--npbar-h) + 24px); }
  .toast .ghost.sm { padding: 4px 12px; color: var(--accent); font-weight: 600; }
  .npmorewrap { position: relative; display: inline-flex; }
  .npmore { position: fixed; z-index: 30; min-width: 220px; padding: 4px; border-radius: 8px; background: var(--bg-hover); box-shadow: 0 8px 24px #000a; transform-origin: bottom right; animation: pop 140ms var(--ease-out); color: var(--text); }
  .npmore button { display: flex; align-items: center; gap: 10px; width: 100%; padding: 8px 10px; border-radius: 6px; text-align: left; }
  .npmore button:hover, .npmore button:focus-visible { background: var(--line2); outline: 0; }
  .npmore span { flex: 1; }
  .npmore b { color: var(--accent); font-variant-numeric: tabular-nums; font-weight: 600; }
  .checker { background: repeating-conic-gradient(#ffffff14 0 25%, transparent 0 50%) 0 0 / 16px 16px; }
  .petlist { display: flex; flex-wrap: wrap; gap: 8px; margin: 12px 0; }
  .petchip { display: flex; align-items: center; gap: 8px; padding: 6px 12px 6px 6px; border-radius: 10px; background: var(--bg-raised); border: 2px solid transparent; }
  .petchip.on { border-color: var(--accent); }
  .petchip:hover { background: var(--bg-hover); }
  .petthumb { width: 40px; height: 40px; border-radius: 6px; display: grid; place-items: center; overflow: hidden; background-color: var(--line2); font-size: 1.4rem; color: var(--text-dim); }
  .petthumb img { width: 100%; height: 100%; object-fit: contain; }
  .petchip.add { color: var(--text-dim); }
  .petedit { display: grid; gap: 12px; padding: 16px; border-radius: 10px; background: var(--bg-raised); max-width: 760px; }
  .petname { display: flex; align-items: center; gap: 10px; color: var(--text-dim); }
  .petname input { flex: 1; max-width: 280px; height: 34px; padding: 0 10px; border-radius: 6px; border: 1px solid var(--line2); background: var(--bg-base); color: var(--text); }
  .petslots { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  .petslot { display: grid; gap: 8px; justify-items: center; padding: 12px; border-radius: 8px; border: 1px solid var(--line2); text-align: center; }
  .petslot.missing { border-style: dashed; }
  .petimg { width: 100%; aspect-ratio: 1; max-height: 150px; border-radius: 6px; display: grid; place-items: center; overflow: hidden; }
  .petimg img { max-width: 100%; max-height: 100%; object-fit: contain; }
  .pettry { display: flex; align-items: center; gap: 12px; }
  .petdemo { position: relative; width: 96px; height: 96px; border-radius: 8px; cursor: pointer; }
  .petdemo img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; visibility: hidden; }
  .petdemo .s1, .petdemo:hover .s2, .petdemo:active .s3 { visibility: visible; }
  .petdemo:hover .s1, .petdemo:active .s2 { visibility: hidden; }
  .petedit .seg, .petedit > .danger { justify-self: start; margin: 0; }
  .petrow { display: flex; flex-wrap: wrap; align-items: end; gap: 16px; }
  .petsize { display: grid; gap: 6px; flex: 1; min-width: 220px; max-width: 360px; }
  .pastebox { margin: 4px 0 8px; }
  .pastebox summary { cursor: pointer; width: fit-content; color: var(--text-dim); padding: 4px 0; }
  .pastebox textarea { width: 100%; max-width: 640px; display: block; margin: 8px 0; padding: 10px 12px; border-radius: 8px; border: 1px solid var(--line2); background: var(--bg-raised); color: var(--text); font: inherit; resize: vertical; }
  .pasterow { display: flex; gap: 8px; max-width: 640px; }
  .pasterow input { flex: 1; height: 38px; padding: 0 12px; border-radius: 8px; border: 1px solid var(--line2); background: var(--bg-raised); color: var(--text); }
  .badge { display: inline-block; margin-left: 6px; padding: 1px 8px; border-radius: 999px; font-size: 0.72rem; font-weight: 600; background: #1db95422; color: #1ed760; vertical-align: 1px; }
  .spsetup { display: grid; gap: 8px; margin: -4px 0 8px; padding: 12px 16px; border-radius: 8px; background: var(--bg-raised); }
  .spsetup label { display: flex; align-items: center; gap: 10px; color: var(--text-dim); }
  .spsetup input { flex: 1; max-width: 360px; height: 34px; padding: 0 10px; border-radius: 6px; border: 1px solid var(--line2); background: var(--bg-base); color: var(--text); font-family: ui-monospace, monospace; }
  .spsetup ol { margin: 8px 0 0; padding-left: 20px; display: grid; gap: 6px; color: var(--text-dim); }
  .spsetup code { user-select: all; }
  .spbox { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin: 4px 0; }
  .sppls { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 8px; margin: 8px 0 12px; }
  .sppl { display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: 8px; background: var(--bg-raised); text-align: left; min-width: 0; }
  .sppl:hover:not(:disabled) { background: var(--bg-hover); }
  .sppl:disabled { opacity: 0.45; cursor: not-allowed; }
  .sppl img, .sppl .thumb { width: 40px; height: 40px; border-radius: 4px; object-fit: cover; flex: none; background: var(--line2); }
  .sppl > span { display: grid; min-width: 0; }
  .sppl b { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spimp { margin: 8px 0 16px; padding: 12px 16px; border-radius: 8px; background: var(--bg-raised); }
  .sphead { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .sphead small { flex: 1; }
  .sppick { display: grid; gap: 6px; padding: 10px 0; border-top: 1px solid var(--line); }
  .spalts { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .spalts .ghost { max-width: 100%; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dlwhy { margin-top: 2px; font-size: 0.8125rem; color: var(--text-dim); }
  .dlwhy summary { cursor: pointer; width: fit-content; }
  .dlwhy ul { margin: 4px 0 0; padding-left: 18px; }
  .nav { user-select: none; }
  .helpbg { position: fixed; inset: 0; z-index: 50; display: grid; place-items: center; background: rgba(0,0,0,.5); }
  .helpbox { background: var(--bg-raised, #1a1b1e); color: var(--text); padding: 20px 24px; border-radius: 12px; min-width: 360px; max-height: 80vh; overflow: auto; }
  .helpbox dl { display: grid; grid-template-columns: auto 1fr; gap: 6px 16px; margin: 12px 0; }
  .helpbox dt { text-align: right; } .helpbox dd { margin: 0; color: var(--text-dim); }
  .helpbox h3 { margin: 14px 0 0; font-size: 0.85rem; color: var(--text-dim); font-weight: 600; }
  .helpbox dl { margin: 6px 0; }
  .keylist { display: grid; grid-template-columns: minmax(150px, max-content) 1fr; gap: 8px 18px; margin: 8px 0 22px; }
  .keylist dt { text-align: right; } .keylist dd { margin: 0; color: var(--text-dim); }
  kbd { padding: 1px 7px; border-radius: 5px; border: 1px solid var(--line, rgba(255,255,255,.2)); font-size: 0.85em; }
  :global(.two b.mq) { display: inline-block; overflow: visible; text-overflow: clip; animation: mq var(--mqt, 4s) linear infinite alternate; }
  @keyframes mq { to { transform: translateX(var(--mq)); } }
  @media (prefers-reduced-motion: reduce) { :global(.two b.mq) { animation: none; } }
  .barly { margin: 0; height: 1.4em; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.9rem; font-weight: 600; text-align: center; color: var(--text); opacity: 0; transition: opacity 200ms var(--ease-out); }
  .barly.on { opacity: 0.9; }
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
  .logo { display: inline-flex; align-items: center; gap: 8px; font-family: 'UnifrakturCook', serif; font-weight: 700; font-size: 1.45rem; letter-spacing: 0.01em; color: var(--text); min-width: calc(var(--sidebar-w) - 24px); text-align: left; }
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
  .thead, .row { display: grid; grid-template-columns: var(--cols, 40px 4fr 2fr 110px 36px 60px 32px); align-items: center; gap: 12px; padding: 0 12px; }
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
  /* เล่น/หยุด: ไอคอนสองอันซ้อนกัน หมุน+ย่อ+เบลอสลับกัน แทนการเปลี่ยนทันที */
  .pp { display: grid; }
  .pp > :global(*) { grid-area: 1 / 1; transition: opacity 180ms var(--ease-out), transform 220ms var(--ease-out), filter 180ms ease; }
  .pp > :global(:last-child), .playing .pp > :global(:first-child) { opacity: 0; transform: rotate(-90deg) scale(0.5); filter: blur(3px); }
  .playing .pp > :global(:last-child) { opacity: 1; transform: none; filter: none; }
  .covwrap { position: relative; width: 56px; height: 56px; flex: none; display: block; }
  .covwrap > .thumb { position: absolute; inset: 0; }
  .breathe { animation: breathe 4.8s ease-in-out infinite; }
  @keyframes breathe { 50% { transform: scale(1.035); } }
  .np-left { position: relative; }
  .np-left > .two { min-width: 0; }
  /* แถวเพลง */
  .row.stag { animation: rowin 300ms var(--ease-out) backwards; animation-delay: calc(var(--d) * 24ms); }
  @keyframes rowin { from { opacity: 0; transform: translateY(8px); } }
  .row { transition: transform 160ms var(--ease-out), opacity 160ms ease, background-color 120ms ease; }
  .row.lifted { opacity: 0.35; transform: scale(0.98); }
  .row.shiftdn { transform: translateY(6px); }
  .row.shiftup { transform: translateY(-6px); }
  /* แถบเวลา/เสียง: เส้นบาง ขยายหนาขึ้นตอนชี้ ปุ่มลากโผล่ตอนชี้ ใหญ่ขึ้นตอนกด */
  input.trk { -webkit-appearance: none; appearance: none; height: 16px; background: transparent; cursor: pointer; }
  input.trk::-webkit-slider-runnable-track { height: 4px; border-radius: 2px; background: linear-gradient(90deg, var(--accent) var(--p), var(--line3) var(--p)); transition: height 140ms var(--ease-out); }
  input.trk:hover::-webkit-slider-runnable-track, input.trk:active::-webkit-slider-runnable-track { height: 6px; }
  input.trk::-webkit-slider-thumb { -webkit-appearance: none; width: 12px; height: 12px; margin-top: -4px; border-radius: 50%; background: var(--text); box-shadow: 0 1px 4px #0008; transform: scale(0); transition: transform 140ms var(--ease-out), margin-top 140ms var(--ease-out); }
  input.trk:hover::-webkit-slider-thumb, input.trk:focus-visible::-webkit-slider-thumb { transform: scale(1); margin-top: -3px; }
  input.trk:active::-webkit-slider-thumb { transform: scale(1.3); margin-top: -3px; }
  input.trk:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; border-radius: 4px; }
  .wave { transition: transform 160ms var(--ease-out); transform-origin: center; }
  .seekwrap:hover .wave { transform: scaleY(1.25); }
  .seektip { animation: tipin 140ms var(--ease-out); }
  @keyframes tipin { from { opacity: 0; transform: translateX(-50%) translateY(3px); } }
  .seekline { position: absolute; top: 2px; bottom: 2px; width: 1px; background: var(--text); opacity: 0.5; pointer-events: none; z-index: 2; }
  .edit[open]::backdrop { animation: bdin 200ms ease; }
  @keyframes bdin { from { opacity: 0; } }
  /* ปิดแอนิเมชัน (ตั้งในแอปหรือ Windows ลดการเคลื่อนไหว) */
  :global(html.nomo) .breathe, :global(html.nomo) .row.stag, :global(html.nomo) .seektip, :global(html.nomo) .dragghost { animation: none; }
  :global(html.nomo) .pp > :global(*), :global(html.nomo) .row, :global(html.nomo) input.trk::-webkit-slider-thumb, :global(html.nomo) .wave { transition: none; }
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

  .tooltabs { display: flex; flex-wrap: wrap; gap: 6px; margin: 0 0 14px; position: sticky; top: 0; z-index: 2; padding: 8px 0; background: var(--bg-base); }
  .tooltabs button { height: 32px; padding: 0 14px; border-radius: 16px; border: 1px solid var(--line); background: transparent; color: var(--dim, var(--text)); cursor: pointer; transition: background-color 150ms ease, color 150ms ease; }
  .tooltabs button:hover { background: var(--bg-raised); color: var(--text); }
  .tooltabs button.on { background: var(--accent); border-color: var(--accent); color: var(--on-accent, #000); }
  .tooltabs button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  .tool.balbox { flex-direction: column; align-items: stretch; gap: 14px; }
  .balbox > .balhead { display: flex; flex-direction: row; align-items: center; gap: 12px; }
  .balhead > div { flex: 1; min-width: 0; }
  .balval { min-width: 84px; text-align: center; padding: 4px 12px; border-radius: 14px; background: var(--accent); color: var(--on-accent, #000); font-weight: 700; font-variant-numeric: tabular-nums; transition: background-color 160ms ease, color 160ms ease; }
  .balval.mid { background: rgba(255, 255, 255, 0.08); color: var(--text); }
  .balbox > .balbar { display: flex; flex-direction: row; align-items: center; gap: 12px; }
  .balhead small { display: block; margin-top: 2px; }
  .balend { display: flex; flex-direction: column; align-items: center; min-width: 36px; font-size: 11.5px; color: var(--text-dim); line-height: 1.2; }
  .balend b { font-size: 16px; color: var(--text); }
  .balrange { position: relative; flex: 1; display: flex; align-items: center; }
  .balrange input { appearance: none; -webkit-appearance: none; width: 100%; height: 24px; background: transparent; cursor: pointer; }
  /* เติมสีจากกึ่งกลางไปยังตำแหน่งปุ่ม (ไม่ใช่จากซ้ายสุด) ให้เห็นชัดว่าเอนไปข้างไหน */
  .balrange input::-webkit-slider-runnable-track { height: 4px; border-radius: 2px; background: linear-gradient(to right, var(--line2, #444) var(--lo), var(--accent) var(--lo), var(--accent) var(--hi), var(--line2, #444) var(--hi)); }
  .balrange input::-webkit-slider-thumb { -webkit-appearance: none; width: 18px; height: 18px; margin-top: -7px; border-radius: 50%; border: 0; background: var(--accent); box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4); transition: transform 120ms ease; }
  .balrange input:active::-webkit-slider-thumb { transform: scale(1.15); }
  .balrange input:focus-visible { outline: 2px solid var(--accent); outline-offset: 4px; border-radius: 4px; }
  .balmid { position: absolute; left: 50%; top: 50%; width: 2px; height: 16px; margin: -8px 0 0 -1px; background: var(--text-dim); opacity: 0.6; border-radius: 1px; pointer-events: none; }
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

  .toast { position: fixed; left: 50%; bottom: 100px; transform: translateX(-50%); z-index: 9; display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-radius: 10px; background: var(--bg-hover); box-shadow: 0 8px 24px #000a; max-width: 90vw; }
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
  .lydlg { width: min(640px, 94vw); max-height: 92vh; overflow-y: auto; }
  .lytools { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 10px; }
  .lyfoot { flex-wrap: wrap; align-items: center; margin-top: 12px; padding-top: 12px; border-top: 1px solid var(--line); }
  .lyfoot .grow { flex: 1; }
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
  .seg.wide { display: inline-flex; margin-bottom: 12px; }
  .seg.wide button { width: auto; padding: 0 14px; height: 32px; }
  .seg.srcseg { margin: 0; align-self: stretch; flex: none; }
  .seg.srcseg button { height: 100%; min-height: 36px; white-space: nowrap; }
  .accbox { display: flex; flex-wrap: wrap; align-items: center; gap: 16px; }
  .bigsw { width: 88px; height: 88px; padding: 0; border: 1px solid var(--line2); border-radius: 10px; background: none; }
  .rgbs { display: grid; gap: 8px; flex: 1; min-width: 240px; max-width: 420px; }
  .rgb { display: grid; grid-template-columns: 36px 1fr 64px; align-items: center; gap: 10px; }
  .rgb > span { color: var(--text-dim); font-variant-numeric: tabular-nums; font-size: 0.85em; }
  .rgb input[type='range'] { appearance: none; -webkit-appearance: none; height: 10px; border-radius: 5px; background: var(--track); }
  .rgb input[type='range']::-webkit-slider-thumb { -webkit-appearance: none; width: 16px; height: 16px; border-radius: 50%; background: #fff; border: 2px solid rgba(0, 0, 0, 0.45); }
  .rgb input[type='number'], .rgb .hex { width: 100%; font-variant-numeric: tabular-nums; }
  .rgb .hex { grid-column: 2 / span 2; max-width: 120px; font-family: ui-monospace, monospace; }
  .saved { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin: 12px 0; }
  .chip { position: relative; display: inline-flex; }
  .chip .dot { width: 30px; height: 30px; border-radius: 50%; border: 2px solid var(--line2); }
  .chip .setbtn { width: 64px; height: 26px; border-radius: 13px; border: 2px solid var(--line2); }
  .chip .on { border-color: var(--text); }
  .chip .x { position: absolute; top: -6px; right: -6px; width: 18px; height: 18px; border-radius: 50%; font-size: 12px; line-height: 1; display: grid; place-items: center; background: var(--bg-raised); color: var(--text-dim); border: 1px solid var(--line2); opacity: 0; transition: opacity 120ms ease; }
  .chip:hover .x, .chip:focus-within .x { opacity: 1; }
  .cyprev { height: 14px; border-radius: 7px; margin-bottom: 12px; max-width: 560px; }
  .stepper { display: flex; align-items: center; gap: 8px; }
  .stepper b { min-width: 1.5em; text-align: center; font-variant-numeric: tabular-nums; }
  .stepper .ghost { width: 32px; padding: 0; }
  .stops { display: flex; flex-wrap: wrap; gap: 8px; }
  .stops input { width: 40px; height: 32px; padding: 0; border: 1px solid var(--line2); border-radius: 6px; background: none; }
  .speed { display: grid; gap: 6px; max-width: 420px; margin-top: 14px; }
  .speed .ends { display: flex; justify-content: space-between; }
  .colors label, .sliders label { display: flex; flex-direction: column; gap: 6px; color: var(--text-dim); }
  .colors input[type='color'] { width: 64px; height: 40px; padding: 0; border: 1px solid var(--line2); border-radius: 6px; background: none; }
  .sliders { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; margin: 16px 0; }
  .sliders output { color: var(--text); margin-left: 6px; }
  .switch { display: flex; gap: 10px; align-items: center; padding: 6px 0; }
  @media (prefers-reduced-motion: reduce) { .heart, .icon.speed { transition: none; } }
</style>