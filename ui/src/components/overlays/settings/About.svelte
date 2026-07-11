<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { onMount } from 'svelte';
  import { api, errorMessage } from '$lib/api';
  import { toast } from '$lib/stores/toast.svelte';
  import Icon from '../../widgets/Icon.svelte';
  import SectionHeader from './SectionHeader.svelte';

  // Reads tauri.conf.json's version, which packaging/set-version.sh stamps
  // from the git tag at release build time (0.0.0 in a dev checkout).
  let version = $state('');

  onMount(async () => {
    version = await getVersion();
  });

  // The release URL is built Rust-side from crate metadata, not passed from
  // here — the webview only triggers the open.
  async function openRelease() {
    try {
      await api.openReleasePage();
    } catch (err) {
      toast.error('Could not open release page', errorMessage(err));
    }
  }
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
      Superpanels{#if version}<button
          type="button"
          class="mono version-link"
          onclick={openRelease}
          title="View this release on GitHub">v{version}</button
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

<style>
  .version-link {
    margin-left: 8px;
    padding: 0;
    border: 0;
    background: none;
    font-size: 12px;
    font-weight: 400;
    color: var(--text-3);
    cursor: pointer;
  }
  .version-link:hover {
    color: var(--accent);
    text-decoration: underline;
  }
</style>
