<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import Main from '$lib/Main.svelte';
  import Mini from '$lib/Mini.svelte';

  // มินิเพลเยอร์คือหน้าต่างที่ชื่อ (label) ว่า "mini" ใช้หน้าเดียวกัน ไม่ต้องพึ่ง query ใน URL
  const isMini = getCurrentWindow().label === 'mini';
</script>

{#if isMini}<Mini />{:else}<Main />{/if}
