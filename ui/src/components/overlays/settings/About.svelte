<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import Icon from '../../widgets/Icon.svelte';
  import SectionHeader from './SectionHeader.svelte';

  // Reads tauri.conf.json's version, which packaging/set-version.sh stamps
  // from the git tag at release build time (0.0.0 in a dev checkout).
  let version = $state('');

  onMount(async () => {
    version = await getVersion();
  });
</script>

<SectionHeader title="About Superpanels" />

<div
  class="flex items-center"
  style:gap="14px"
  style:padding="16px"
  style:background="var(--bg-2)"
  style:border-radius="8px"
  style:border="1px solid var(--line)"
>
  <Icon name="logo-lg" size={48} />
  <div>
    <div style:font-size="18px" style:font-weight="600">
      Superpanels{#if version}<span
          class="mono"
          style:font-size="12px"
          style:font-weight="400"
          style:color="var(--text-3)"
          style:margin-left="8px">v{version}</span
        >{/if}
    </div>
    <div class="mono" style:font-size="11px" style:color="var(--text-3)" style:margin-top="4px">
      tauri 2 · svelte 5 · rust · linux
    </div>
    <div style:font-size="11px" style:color="var(--text-3)" style:margin-top="6px">
      Bezel-aware multi-monitor wallpapers for Linux.
    </div>
  </div>
</div>
