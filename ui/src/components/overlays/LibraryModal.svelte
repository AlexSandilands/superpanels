<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import type { LibraryEntry } from '$lib/api';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { monitorStore } from '$lib/stores/monitors.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import { stableId } from '$lib/canvas/previewLayout';
  import Backdrop from '../chrome/Backdrop.svelte';
  import StepperInput from '../chrome/StepperInput.svelte';
  import Icon from '../Icon.svelte';
  import LibraryThumb from './LibraryThumb.svelte';

  type AspectFilter = 'all' | 'wide' | 'square' | 'portrait';
  type SortKey = 'date_added' | 'date_modified' | 'resolution' | 'last_shown' | 'name';
  const aspectOptions: { value: AspectFilter; label: string }[] = [
    { value: 'all', label: 'Any' },
    { value: 'wide', label: 'Wide' },
    { value: 'square', label: 'Square' },
    { value: 'portrait', label: 'Portrait' },
  ];
  const sortOptions: { value: SortKey; label: string }[] = [
    { value: 'date_added', label: 'Added' },
    { value: 'date_modified', label: 'Modified' },
    { value: 'resolution', label: 'Resolution' },
    { value: 'last_shown', label: 'Last shown' },
    { value: 'name', label: 'Name' },
  ];

  type Props = {
    onClose: () => void;
    onApplyAsSpan: (path: string) => void;
    onPinToMonitor: (monitorId: string, path: string) => void;
  };
  let { onClose, onApplyAsSpan, onPinToMonitor }: Props = $props();

  let pinFor = $state<string | null>(null);

  let searchEl: HTMLInputElement | undefined = $state();
  let scrollEl: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportH = $state(0);
  let viewportW = $state(0);

  const TILE_MIN_PX = 220;
  const ROW_GAP_PX = 12;
  const ROW_HEIGHT_PX = 178;

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
    visible.slice(startRow * cols, endRow * cols).map((entry, i) => {
      const idx = startRow * cols + i;
      return { entry, row: Math.floor(idx / cols), col: idx % cols };
    }),
  );

  onMount(() => {
    void libraryStore.refresh();
    if (searchEl) searchEl.focus();
    if (!scrollEl) return;
    const ro = new ResizeObserver(() => measure());
    ro.observe(scrollEl);
    measure();
    return () => ro.disconnect();
  });

  function measure() {
    if (!scrollEl) return;
    const r = scrollEl.getBoundingClientRect();
    viewportH = r.height;
    viewportW = r.width;
  }

  function onScroll() {
    if (!scrollEl) return;
    scrollTop = scrollEl.scrollTop;
  }

  async function pickFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') await libraryStore.addRoot(picked);
  }

  function applyEntry(entry: LibraryEntry) {
    onApplyAsSpan(entry.path);
    onClose();
  }

  function onDragStart(ev: DragEvent, entry: LibraryEntry) {
    if (!ev.dataTransfer) return;
    ev.dataTransfer.effectAllowed = 'copy';
    ev.dataTransfer.setData('application/x-superpanels-image', entry.path);
    ev.dataTransfer.setData('text/plain', entry.path);
  }

  function aspectLabel(ratio: number): string {
    if (ratio > 2.5) return '32:9';
    if (ratio > 1.6) return '16:9';
    if (ratio > 1.2) return '4:3';
    if (ratio > 0.8) return '1:1';
    return '9:16';
  }
</script>

<Backdrop {onClose}>
  <div
    class="panel flex flex-col overflow-hidden"
    style:width="min(1100px, 92vw)"
    style:height="min(720px, 88vh)"
  >
    <!-- Toolbar -->
    <div
      class="flex items-center"
      style:gap="12px"
      style:padding="12px 16px"
      style:border-bottom="1px solid var(--line)"
    >
      <div style:font-size="14px" style:font-weight="600">Library</div>
      <div
        class="flex items-center"
        style:gap="6px"
        style:background="var(--bg-2)"
        style:border="1px solid var(--line)"
        style:border-radius="6px"
        style:height="28px"
        style:padding="0 10px"
        style:flex="1"
        style:max-width="360px"
      >
        <span style:color="var(--text-3)"><Icon name="search" size={11} /></span>
        <input
          bind:this={searchEl}
          bind:value={libraryStore.search}
          placeholder="Search images, tags…"
          style:flex="1"
          style:background="transparent"
          style:border="none"
          style:outline="none"
          style:color="var(--text)"
          style:font-size="12px"
        />
        <span class="kbd">⌃L</span>
      </div>
      <button
        class="chip"
        class:active={libraryStore.favouritesOnly}
        onclick={() => (libraryStore.favouritesOnly = !libraryStore.favouritesOnly)}
      >
        <Icon name={libraryStore.favouritesOnly ? 'star-filled' : 'star'} size={11} /> favourites
      </button>
      <div style:flex="1"></div>
      <button class="btn ghost icon" onclick={onClose} aria-label="Close">×</button>
    </div>

    <div class="flex" style:flex="1" style:min-height="0">
      <!-- Roots sidebar -->
      <div
        style:width="220px"
        style:padding="14px"
        style:border-right="1px solid var(--line)"
        style:overflow="auto"
      >
        <div class="section-label">Roots</div>
        {#each libraryStore.roots as root (root)}
          <div class="root-row">
            <Icon name="folder" />
            <div style:flex="1" style:min-width="0">
              <div
                class="mono"
                style:font-size="11px"
                style:overflow="hidden"
                style:text-overflow="ellipsis"
                style:white-space="nowrap"
                style:color="var(--text-2)"
              >
                {root}
              </div>
            </div>
            <button
              class="btn ghost sm"
              style:padding="0 6px"
              title="Remove"
              onclick={() => void libraryStore.removeRoot(root)}
              disabled={libraryStore.busyRoots}
            >
              ×
            </button>
          </div>
        {/each}
        <button
          class="btn ghost sm"
          style:width="100%"
          style:margin-top="8px"
          style:justify-content="flex-start"
          onclick={() => void pickFolder()}
          disabled={libraryStore.busyRoots}
        >
          <Icon name="plus" size={11} /> Add root
        </button>

        <div class="section-label" style:margin-top="18px">Filters</div>
        <div class="filter-row">
          <span>Min px</span>
          <StepperInput
            value={libraryStore.minResolution}
            unit="px"
            step={100}
            bigStep={500}
            min={0}
            max={16000}
            decimals={0}
            width={56}
            onChange={(v) => (libraryStore.minResolution = v)}
          />
        </div>
        <div class="filter-row" style:flex-direction="column" style:align-items="stretch">
          <span style:margin-bottom="6px">Aspect</span>
          <div class="seg">
            {#each aspectOptions as o (o.value)}
              <button
                class:seg-active={libraryStore.aspect === o.value}
                onclick={() => (libraryStore.aspect = o.value)}
              >
                {o.label}
              </button>
            {/each}
          </div>
        </div>

        <div class="section-label" style:margin-top="18px">Sort by</div>
        <div class="flex" style:gap="4px" style:flex-wrap="wrap">
          {#each sortOptions as o (o.value)}
            <button
              class="chip"
              class:active={libraryStore.sort === o.value}
              onclick={() => (libraryStore.sort = o.value)}
            >
              {o.label}
            </button>
          {/each}
        </div>

        {#if libraryStore.tags.length > 0}
          <div class="section-label" style:margin-top="18px">Tags</div>
          <div class="flex" style:gap="4px" style:flex-wrap="wrap">
            <button
              class="chip"
              class:active={libraryStore.activeTag === null}
              onclick={() => (libraryStore.activeTag = null)}
            >
              any
            </button>
            {#each libraryStore.tags as tag (tag)}
              <button
                class="chip"
                class:active={libraryStore.activeTag === tag}
                onclick={() =>
                  (libraryStore.activeTag = libraryStore.activeTag === tag ? null : tag)}
              >
                {tag}
              </button>
            {/each}
          </div>
        {/if}

        <button
          class="btn sm"
          style:width="100%"
          style:margin-top="14px"
          onclick={() => void libraryStore.rescan()}
          disabled={libraryStore.loading}
        >
          <Icon name="refresh" size={12} />
          {libraryStore.loading ? 'Scanning…' : 'Rescan'}
        </button>

        {#if libraryStore.loading}
          <div class="progress-card">
            <div style:font-size="11px" style:font-weight="500" style:margin-bottom="4px">
              Indexing…
            </div>
            <div class="progress-track">
              <div class="progress-bar"></div>
            </div>
            <div
              class="mono"
              style:font-size="10px"
              style:color="var(--text-3)"
              style:margin-top="4px"
            >
              {libraryStore.entries.length} so far
            </div>
          </div>
        {/if}
      </div>

      <!-- Grid -->
      <div
        bind:this={scrollEl}
        class="scroll"
        style:flex="1"
        style:padding="14px"
        onscroll={onScroll}
      >
        <div
          class="mono"
          style:font-size="11px"
          style:color="var(--text-3)"
          style:margin-bottom="10px"
        >
          {visible.length} of {libraryStore.entries.length} images
        </div>
        {#if visible.length === 0}
          <div style:padding="14px" style:font-size="12px" style:color="var(--text-3)">
            {#if libraryStore.entries.length === 0}
              {#if libraryStore.roots.length === 0}
                No library roots configured yet — add one on the left.
              {:else}
                Roots configured but no images indexed. Try Rescan.
              {/if}
            {:else}
              No matches for the current filters.
            {/if}
          </div>
        {:else}
          <div class="relative w-full" style:height="{totalH}px">
            {#each visibleRange as item (item.entry.path)}
              <div
                class="lib-card absolute"
                style:top="{item.row * (ROW_HEIGHT_PX + ROW_GAP_PX)}px"
                style:left="calc({(item.col * 100) / cols}% + {ROW_GAP_PX / 2}px)"
                style:width="calc({100 / cols}% - {ROW_GAP_PX}px)"
                style:height="{ROW_HEIGHT_PX}px"
                draggable="true"
                ondragstart={(ev) => onDragStart(ev, item.entry)}
                ondblclick={() => applyEntry(item.entry)}
                title={item.entry.path}
                role="button"
                tabindex="0"
              >
                <div style:flex="1" style:min-height="0" style:position="relative">
                  <LibraryThumb path={item.entry.path} alt={item.entry.path} />
                  <button
                    class="fav"
                    style:color={item.entry.favourite ? 'var(--warn)' : 'oklch(1 0 0 / 0.7)'}
                    onclick={(ev) => {
                      ev.stopPropagation();
                      void libraryStore.toggleFavourite(item.entry.path);
                    }}
                    aria-label="Toggle favourite"
                  >
                    <Icon name={item.entry.favourite ? 'star-filled' : 'star'} />
                  </button>
                  <div class="aspect-tag mono">{aspectLabel(item.entry.aspect_ratio)}</div>
                </div>
                <div style:padding="8px 10px">
                  <div
                    style:font-size="11px"
                    style:font-weight="500"
                    style:overflow="hidden"
                    style:text-overflow="ellipsis"
                    style:white-space="nowrap"
                  >
                    {item.entry.path.split('/').pop()}
                  </div>
                  <div
                    class="mono flex"
                    style:font-size="10px"
                    style:color="var(--text-3)"
                    style:margin-top="2px"
                    style:justify-content="space-between"
                  >
                    <span>{item.entry.resolution[0]}×{item.entry.resolution[1]}</span>
                    <span>{item.entry.tags.slice(0, 2).join(' · ')}</span>
                  </div>
                  <div class="flex" style:gap="4px" style:margin-top="8px">
                    <button
                      class="btn primary sm"
                      style:flex="1"
                      style:font-size="10px"
                      onclick={(ev) => {
                        ev.stopPropagation();
                        applyEntry(item.entry);
                      }}
                    >
                      Apply
                    </button>
                    <button
                      class="btn sm"
                      style:padding="0 6px"
                      title="Set for monitor…"
                      onclick={(ev) => {
                        ev.stopPropagation();
                        pinFor = pinFor === item.entry.path ? null : item.entry.path;
                      }}
                    >
                      <Icon name="link" size={11} />
                    </button>
                    <button
                      class="btn sm"
                      style:padding="0 6px"
                      title="Reveal in file manager"
                      onclick={(ev) => {
                        ev.stopPropagation();
                        toast.info('Reveal', 'not yet wired in this build');
                      }}
                    >
                      <Icon name="reveal" size={11} />
                    </button>
                  </div>
                  {#if pinFor === item.entry.path}
                    <div class="pin-pop">
                      <div class="section-label" style:margin-bottom="4px">Pin to…</div>
                      {#if monitorStore.monitors.length === 0}
                        <div style:font-size="11px" style:color="var(--text-3)">
                          No monitors detected.
                        </div>
                      {:else}
                        {#each monitorStore.monitors as m (stableId(m))}
                          <button
                            class="pin-row"
                            onclick={(ev) => {
                              ev.stopPropagation();
                              onPinToMonitor(stableId(m), item.entry.path);
                              pinFor = null;
                            }}
                          >
                            <span class="mono" style:font-size="11px">{m.name}</span>
                            <span
                              class="mono"
                              style:font-size="10px"
                              style:color="var(--text-3)"
                              style:margin-left="auto"
                            >
                              {m.resolution[0]}×{m.resolution[1]}
                            </span>
                          </button>
                        {/each}
                      {/if}
                    </div>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</Backdrop>

<style>
  .section-label {
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--text-3);
    text-transform: uppercase;
    margin-bottom: 8px;
  }
  .root-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 4px;
    margin-bottom: 2px;
    color: var(--text-3);
  }
  .root-row:hover {
    background: var(--panel-2);
  }
  .filter-row {
    display: flex;
    justify-content: space-between;
    padding: 4px 8px;
    font-size: 11px;
    align-items: center;
    color: var(--text-3);
    margin-bottom: 6px;
  }
  .seg {
    display: inline-flex;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--line);
    width: 100%;
  }
  .seg button {
    flex: 1;
    border: none;
    height: 26px;
    padding: 0 8px;
    font-size: 11px;
    font-weight: 500;
    background: transparent;
    color: var(--text-2);
    font-family: inherit;
  }
  .seg button:hover {
    background: var(--panel-2);
  }
  .seg-active {
    background: var(--accent) !important;
    color: oklch(0.16 0.01 250) !important;
  }
  .lib-card {
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid var(--line);
    background: var(--panel-2);
    text-align: left;
    color: inherit;
    display: flex;
    flex-direction: column;
    padding: 0;
  }
  .lib-card:hover {
    border-color: var(--accent);
  }
  .fav {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 24px;
    height: 24px;
    border-radius: 4px;
    background: oklch(0 0 0 / 0.4);
    border: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .aspect-tag {
    position: absolute;
    bottom: 6px;
    left: 8px;
    font-size: 10px;
    color: oklch(1 0 0 / 0.85);
    text-shadow: 0 1px 2px oklch(0 0 0 / 0.5);
  }
  .pin-pop {
    margin-top: 6px;
    padding: 8px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .pin-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px;
    border: none;
    background: transparent;
    color: inherit;
    border-radius: 4px;
    text-align: left;
  }
  .pin-row:hover {
    background: var(--panel-2);
  }
  .progress-card {
    margin-top: 14px;
    padding: 10px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .progress-track {
    height: 4px;
    background: var(--line);
    border-radius: 2px;
    overflow: hidden;
    position: relative;
  }
  .progress-bar {
    position: absolute;
    inset: 0;
    width: 40%;
    background: var(--accent);
    animation: indeterminate 1.4s ease-in-out infinite;
  }
  @keyframes indeterminate {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(250%);
    }
  }
</style>
