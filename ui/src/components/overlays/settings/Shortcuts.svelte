<script lang="ts">
  import SectionHeader from './SectionHeader.svelte';

  const shortcuts: Array<[string, string]> = [
    ['Apply', '↵'],
    ['Switch profile 1 / 2 / 3', '⌃1 · ⌃2 · ⌃3'],
    ['Pause / resume slideshow', 'Space'],
    ['Next / previous slide', '→ / ←'],
    ['Reset image transform', 'R'],
    ['Toggle off-monitor dim', 'D'],
    ['Re-detect monitors', 'F5'],
    ['Settings', '⌃,'],
    ['Library', '⌃L'],
    ['Rotate selected monitor', '[ / ]'],
    ['Nudge selected monitor', '←↑→↓'],
    ['Big nudge', '⇧ + arrow'],
    ['Close modal', 'Esc'],
  ];
</script>

<SectionHeader title="Keyboard shortcuts" />

<div style:display="grid" style:grid-template-columns="1fr 1fr" style:gap="4px 32px">
  {#each shortcuts as [label, key] (label)}
    <div class="kvrow">
      <span style:font-size="12px">{label}</span>
      <span class="kbd">{key}</span>
    </div>
  {/each}
</div>

<style>
  .kvrow {
    display: flex;
    justify-content: space-between;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
</style>
