<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Main from '$lib/Main.svelte';
  import Mini from '$lib/Mini.svelte';
  import LyFloat from '$lib/LyFloat.svelte';

  // มินิเพลเยอร์คือหน้าต่างที่ชื่อ (label) ว่า "mini" ใช้หน้าเดียวกัน ไม่ต้องพึ่ง query ใน URL
  const label = getCurrentWindow().label;
</script>

{#if label === 'mini'}<Mini />{:else if label === 'lyrics'}<LyFloat />{:else}<Main />{/if}
