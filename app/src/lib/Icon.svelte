<script lang="ts">
  let { n, size = 20, fill = false }: { n: string; size?: number; fill?: boolean } = $props();

  // ไอคอนเส้นง่ายๆ วาดเอง (ไม่ใช้ไอคอนของ Spotify)
  const d: Record<string, string> = {
    play: 'M7 4l13 8-13 8z',
    pause: 'M6 4h4v16H6z|M14 4h4v16h-4z',
    prev: 'M19 5v14L9 12z|M5 5v14',
    next: 'M5 5v14l10-7z|M19 5v14',
    shuffle: 'M16 3h5v5|M4 20L21 3|M21 16v5h-5|M15 15l6 6|M4 4l5 5',
    repeat: 'M17 2l4 4-4 4|M3 11v-1a4 4 0 0 1 4-4h14|M7 22l-4-4 4-4|M21 13v1a4 4 0 0 1-4 4H3',
    volume: 'M11 5L6 9H2v6h4l5 4z|M15.5 8.5a5 5 0 0 1 0 7|M19 5a10 10 0 0 1 0 14',
    search: 'M21 21l-4.3-4.3|M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16z',
    home: 'M3 11l9-8 9 8|M5 10v10h14V10|M10 20v-6h4v6',
    songs: 'M9 18V5l12-2v13|M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0z|M21 16a3 3 0 1 1-6 0 3 3 0 0 1 6 0z',
    download: 'M12 3v12|M7 10l5 5 5-5|M5 21h14',
    more: 'M5 12h.01|M12 12h.01|M19 12h.01',
    queue: 'M8 6h13|M8 12h13|M8 18h13|M3 6h.01|M3 12h.01|M3 18h.01',
    plus: 'M12 5v14|M5 12h14',
    sidebar: 'M3 4h18v16H3z|M9 4v16',
    lyrics: 'M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z|M8 9h8|M8 13h5',
    moon: 'M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z',
    mini: 'M3 5h18v14H3z|M12 12h9v7h-9z',
    tools: 'M14.7 6.3a4 4 0 0 0-5 5L3 18l3 3 6.7-6.7a4 4 0 0 0 5-5l-2.4 2.4-2.6-.6-.6-2.6z',
    trash: 'M4 7h16|M10 11v6|M14 11v6|M6 7l1 13h10l1-13|M9 7V4h6v3',
    bolt: 'M13 2L4 14h7l-1 8 9-12h-7z',
    heart: 'M12 21s-7-4.4-9.3-9A5.2 5.2 0 0 1 12 6a5.2 5.2 0 0 1 9.3 6c-2.3 4.6-9.3 9-9.3 9z',
    palette: 'M12 3a9 9 0 1 0 0 18c1.1 0 1.6-.8 1.6-1.6 0-.5-.3-1-.3-1.5 0-.9.7-1.4 1.5-1.4H17a4 4 0 0 0 4-4c0-4.4-4-8.5-9-8.5z|M7.5 11h.01|M10 7.5h.01|M14.5 7.5h.01'
  };
  const filled = ['play', 'pause', 'prev', 'next'];
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={fill || filled.includes(n) ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#each d[n].split('|') as p}<path d={p} />{/each}
</svg>
