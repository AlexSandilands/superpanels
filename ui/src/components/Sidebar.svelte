<script lang="ts">
  type Tab = 'profiles' | 'library' | 'settings';

  let { tab = $bindable('profiles' as Tab) }: { tab: Tab } = $props();

  const items: Array<{ id: Tab; label: string }> = [
    { id: 'profiles', label: 'Profiles' },
    { id: 'library', label: 'Library' },
    { id: 'settings', label: 'Settings' },
  ];
</script>

<aside
  class="flex flex-col gap-1 border-r border-slate-800 bg-slate-900/40 p-3"
  style="width: var(--rail-width)"
>
  <h1 class="px-2 pb-3 text-sm font-semibold text-slate-300">Superpanels</h1>
  {#each items as item (item.id)}
    <button
      type="button"
      class="rounded px-3 py-2 text-left text-sm hover:bg-slate-800/60"
      class:bg-slate-800={tab === item.id}
      class:text-accent={tab === item.id}
      onclick={() => (tab = item.id)}
    >
      {item.label}
    </button>
  {/each}
</aside>
