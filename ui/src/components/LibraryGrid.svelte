<script lang="ts">
  // SPEC §7 / §12.2 — virtualised library grid. Only visible rows are mounted
  // to keep DOM cost flat for thousands of entries. Drag-source for the
  // canvas's drop-onto-monitor flow.

  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import type { LibraryEntry } from '$lib/api';
  import { libraryStore } from '$lib/stores/library.svelte';
  import LibraryThumb from './library/LibraryThumb.svelte';

  async function pickAndAddFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') await libraryStore.addRoot(picked);
  }

  type Props = {
    onApplyAsSpan?: (path: string) => void;
  };
  let { onApplyAsSpan }: Props = $props();

  let scrollEl: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportH = $state(0);
  let viewportW = $state(0);

  const TILE_MIN_PX = 180;
  const ROW_GAP_PX = 8;
  const ROW_HEIGHT_PX = 152;

  const visible = $derived(libraryStore.visible);

  const cols = $derived(
    Math.max(1, Math.floor((viewportW + ROW_GAP_PX) / (TILE_MIN_PX + ROW_GAP_PX))),
  );
  const totalRows = $derived(Math.ceil(visible.length / cols));
  const totalH = $derived(totalRows * (ROW_HEIGHT_PX + ROW_GAP_PX));
  const startRow = $derived(Math.max(0, Math.floor(scrollTop / (ROW_HEIGHT_PX + ROW_GAP_PX)) - 1));
  const endRow = $derived(
    Math.min(totalRows, Math.ceil((scrollTop + viewportH) / (ROW_HEIGHT_PX + ROW_GAP_PX)) + 1),
  );
  const visibleRange = $derived(
    visible.slice(startRow * cols, endRow * cols).map((e, i) => {
      const idx = startRow * cols + i;
      return { entry: e, row: Math.floor(idx / cols), col: idx % cols };
    }),
  );

  let menu = $state<{
    x: number;
    y: number;
    entry: LibraryEntry;
  } | null>(null);

  let tagInputFor = $state<LibraryEntry | null>(null);
  let tagInput = $state('');

  onMount(() => {
    void libraryStore.refresh();
    if (!scrollEl) return;
    const ro = new ResizeObserver(() => measure());
    ro.observe(scrollEl);
    measure();
    return () => ro.disconnect();
  });

  function measure() {
    if (!scrollEl) return;
    const rect = scrollEl.getBoundingClientRect();
    viewportH = rect.height;
    viewportW = rect.width;
  }

  function onScroll() {
    if (!scrollEl) return;
    scrollTop = scrollEl.scrollTop;
  }

  function onDragStart(ev: DragEvent, entry: LibraryEntry) {
    if (!ev.dataTransfer) return;
    ev.dataTransfer.effectAllowed = 'copy';
    // Two formats: a custom one (used by canvas drop), and text/uri-list as a
    // sane fallback for any other drop target.
    ev.dataTransfer.setData('application/x-superpanels-image', entry.path);
    ev.dataTransfer.setData('text/uri-list', `file://${entry.path}`);
    ev.dataTransfer.setData('text/plain', entry.path);
  }

  function openMenu(ev: MouseEvent, entry: LibraryEntry) {
    ev.preventDefault();
    menu = { x: ev.clientX, y: ev.clientY, entry };
  }

  function closeMenu() {
    menu = null;
  }

  function handleApplyAsSpan(entry: LibraryEntry) {
    onApplyAsSpan?.(entry.path);
    closeMenu();
  }

  async function handleToggleFavourite(entry: LibraryEntry) {
    closeMenu();
    await libraryStore.toggleFavourite(entry.path);
  }

  function startTagFlow(entry: LibraryEntry) {
    tagInputFor = entry;
    tagInput = '';
    closeMenu();
  }

  async function commitTag() {
    if (!tagInputFor) return;
    const v = tagInput.trim();
    const target = tagInputFor;
    tagInputFor = null;
    if (!v) return;
    await libraryStore.setTag(target.path, v, true);
  }

  async function handleDelete(entry: LibraryEntry) {
    closeMenu();
    await libraryStore.remove(entry.path);
  }
</script>

<svelte:window onclick={closeMenu} />

<div class="flex h-full min-h-0 flex-col gap-2">
  <div class="flex flex-wrap items-center gap-2 text-xs">
    <input
      type="search"
      class="flex-1 min-w-[140px] rounded border border-slate-700 bg-slate-900/60 px-2 py-1 text-slate-100"
      placeholder="Search filename or tag…"
      bind:value={libraryStore.search}
    />
    <select
      class="rounded border border-slate-700 bg-slate-900/60 px-2 py-1 text-slate-200"
      bind:value={libraryStore.aspect}
    >
      <option value="all">All aspects</option>
      <option value="wide">Wide</option>
      <option value="square">Square</option>
      <option value="portrait">Portrait</option>
    </select>
    <label class="flex items-center gap-1 text-slate-300">
      Min&nbsp;px
      <input
        type="number"
        min="0"
        step="100"
        class="w-20 rounded border border-slate-700 bg-slate-900/60 px-2 py-1 text-slate-100"
        bind:value={libraryStore.minResolution}
      />
    </label>
    <select
      class="rounded border border-slate-700 bg-slate-900/60 px-2 py-1 text-slate-200"
      bind:value={libraryStore.sort}
    >
      <option value="date_added">Date added</option>
      <option value="date_modified">Date modified</option>
      <option value="resolution">Resolution</option>
      <option value="last_shown">Last shown</option>
      <option value="name">Name</option>
    </select>
    <label class="flex items-center gap-1 text-slate-300">
      <input type="checkbox" bind:checked={libraryStore.favouritesOnly} />
      Favourites
    </label>
    <button
      type="button"
      class="rounded border border-slate-700 px-2 py-1 hover:bg-slate-800"
      onclick={() => void libraryStore.rescan()}
      disabled={libraryStore.loading}
    >
      {libraryStore.loading ? 'Scanning…' : 'Rescan'}
    </button>
    <button
      type="button"
      class="rounded border border-accent/60 bg-accent/10 px-2 py-1 text-accent hover:bg-accent/20"
      onclick={() => void pickAndAddFolder()}
      disabled={libraryStore.busyRoots}
    >
      {libraryStore.busyRoots ? 'Adding…' : 'Add folder…'}
    </button>
  </div>

  {#if libraryStore.tags.length > 0}
    <div class="flex flex-wrap items-center gap-1 text-xs">
      <span class="text-slate-400">Tags:</span>
      <button
        type="button"
        class="rounded-full border border-slate-700 px-2 py-0.5 hover:bg-slate-800"
        class:bg-accent={libraryStore.activeTag === null}
        class:text-slate-900={libraryStore.activeTag === null}
        onclick={() => (libraryStore.activeTag = null)}
      >
        any
      </button>
      {#each libraryStore.tags as tag (tag)}
        <button
          type="button"
          class="rounded-full border border-slate-700 px-2 py-0.5 hover:bg-slate-800"
          class:bg-accent={libraryStore.activeTag === tag}
          class:text-slate-900={libraryStore.activeTag === tag}
          onclick={() => (libraryStore.activeTag = libraryStore.activeTag === tag ? null : tag)}
        >
          {tag}
        </button>
      {/each}
    </div>
  {/if}

  <div
    bind:this={scrollEl}
    class="relative min-h-0 flex-1 overflow-auto rounded border border-slate-800 bg-slate-950/60"
    onscroll={onScroll}
  >
    {#if visible.length === 0}
      {#if libraryStore.entries.length === 0}
        <div class="flex flex-col items-start gap-2 p-4 text-xs text-slate-400">
          {#if libraryStore.roots.length === 0}
            <p>No library roots configured yet.</p>
            <button
              type="button"
              class="rounded border border-accent/60 bg-accent/10 px-2 py-1 text-accent hover:bg-accent/20"
              onclick={() => void pickAndAddFolder()}
              disabled={libraryStore.busyRoots}
            >
              {libraryStore.busyRoots ? 'Adding…' : 'Pick a folder to scan'}
            </button>
          {:else}
            <p>
              Roots configured but no images indexed. Try
              <button
                type="button"
                class="underline hover:text-accent"
                onclick={() => void libraryStore.rescan()}>Rescan</button
              >.
            </p>
          {/if}
        </div>
      {:else}
        <p class="p-4 text-xs text-slate-500">No matches for the current filters.</p>
      {/if}
    {:else}
      <div class="relative w-full" style:height={`${totalH}px`}>
        {#each visibleRange as item (item.entry.path)}
          <div
            class="absolute"
            style:top={`${item.row * (ROW_HEIGHT_PX + ROW_GAP_PX)}px`}
            style:left={`calc(${(item.col * 100) / cols}% + ${ROW_GAP_PX / 2}px)`}
            style:width={`calc(${100 / cols}% - ${ROW_GAP_PX}px)`}
            style:height={`${ROW_HEIGHT_PX}px`}
          >
            <button
              type="button"
              class="group relative h-full w-full overflow-hidden rounded border border-slate-800 bg-slate-900/40 text-left hover:border-accent"
              draggable="true"
              ondragstart={(ev) => onDragStart(ev, item.entry)}
              oncontextmenu={(ev) => openMenu(ev, item.entry)}
              ondblclick={() => onApplyAsSpan?.(item.entry.path)}
              title={item.entry.path}
            >
              <LibraryThumb path={item.entry.path} alt={item.entry.path} />
              {#if item.entry.favourite}
                <span
                  class="pointer-events-none absolute right-1 top-1 rounded bg-amber-400/90 px-1 text-[10px] font-bold text-slate-900"
                  >★</span
                >
              {/if}
              <span
                class="pointer-events-none absolute inset-x-0 bottom-0 truncate bg-slate-950/80 px-1.5 py-0.5 text-[10px] text-slate-200"
              >
                {item.entry.path.split('/').pop()}
              </span>
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <p class="text-[11px] text-slate-500">
    {visible.length} of {libraryStore.entries.length} entries
  </p>
</div>

{#if menu}
  <!-- reason: stopPropagation on container blocks the window-level click that
       closes the menu; children are real <button>s and carry the keyboard
       semantics, so the container itself doesn't need its own keyboard
       handler. -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    role="menu"
    class="fixed z-30 min-w-[180px] rounded border border-slate-700 bg-slate-900 p-1 text-xs shadow-lg"
    style:left={`${menu.x}px`}
    style:top={`${menu.y}px`}
    onclick={(ev) => ev.stopPropagation()}
  >
    <button
      type="button"
      class="block w-full rounded px-2 py-1 text-left hover:bg-slate-800"
      onclick={() => menu && handleApplyAsSpan(menu.entry)}
    >
      Apply as span source
    </button>
    <button
      type="button"
      class="block w-full rounded px-2 py-1 text-left hover:bg-slate-800"
      onclick={() => menu && handleToggleFavourite(menu.entry)}
    >
      {menu.entry.favourite ? 'Unfavourite' : 'Favourite'}
    </button>
    <button
      type="button"
      class="block w-full rounded px-2 py-1 text-left hover:bg-slate-800"
      onclick={() => menu && startTagFlow(menu.entry)}
    >
      Add tag…
    </button>
    <button
      type="button"
      class="block w-full rounded px-2 py-1 text-left text-rose-300 hover:bg-rose-900/40"
      onclick={() => menu && handleDelete(menu.entry)}
    >
      Remove from library
    </button>
  </div>
{/if}

{#if tagInputFor}
  <div
    class="fixed inset-0 z-40 flex items-center justify-center bg-slate-950/60"
    onclick={() => (tagInputFor = null)}
    role="presentation"
  >
    <div
      class="w-[300px] rounded border border-slate-700 bg-slate-900 p-3 text-xs"
      onclick={(ev) => ev.stopPropagation()}
      role="presentation"
    >
      <p class="mb-2 text-slate-300">
        Tag <span class="text-slate-100">{tagInputFor.path.split('/').pop()}</span>:
      </p>
      <input
        type="text"
        class="mb-2 w-full rounded border border-slate-700 bg-slate-800 px-2 py-1 text-slate-100"
        placeholder="tag name"
        bind:value={tagInput}
        onkeydown={(ev) => {
          if (ev.key === 'Enter') void commitTag();
          if (ev.key === 'Escape') tagInputFor = null;
        }}
      />
      <div class="flex justify-end gap-2">
        <button
          type="button"
          class="rounded border border-slate-700 px-2 py-1 hover:bg-slate-800"
          onclick={() => (tagInputFor = null)}>Cancel</button
        >
        <button
          type="button"
          class="rounded bg-accent px-2 py-1 text-slate-900 hover:bg-accent/90"
          onclick={() => void commitTag()}>Save</button
        >
      </div>
    </div>
  </div>
{/if}
