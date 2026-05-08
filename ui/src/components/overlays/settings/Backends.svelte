<script lang="ts">
  import { runtime } from '$lib/stores/runtime.svelte';
  import SectionHeader from './SectionHeader.svelte';
</script>

<SectionHeader
  title="Backends"
  sub="The mechanism Superpanels uses to push the wallpaper to your compositor."
/>

<div class="row">
  <span class="dot live"></span>
  <div style:flex="1">
    <div style:font-size="13px" style:font-weight="500" style:display="flex" style:gap="8px">
      {runtime.last?.backend ?? 'auto-detect'}
      <span class="chip active">ACTIVE</span>
    </div>
    <div class="mono" style:font-size="10px" style:color="var(--text-3)" style:margin-top="2px">
      The currently active backend, last seen
      {runtime.last ? `${Math.round((Date.now() - runtime.last.at) / 1000)}s ago` : 'never'}.
    </div>
  </div>
</div>

<div style:margin-top="14px" style:font-size="11px" style:color="var(--text-3)">
  Pinning a backend manually (KDE / GNOME / Sway / wlroots / feh / custom shell command) is not yet
  wired in this build — the daemon picks the best match for your session. To force one, set
  <code class="mono">backend</code> in the config file.
</div>

<style>
  .row {
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 12px;
  }
</style>
