<script lang="ts">
  import type { LibraryEntry } from '$lib/api';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import Icon from '../widgets/Icon.svelte';
  import LibraryThumb from './LibraryThumb.svelte';
  import LibraryPinMenu from './LibraryPinMenu.svelte';

  export type SlideshowSelection = {
    membershipOf: (path: string) => 'image' | 'folder' | null;
    onToggle: (path: string) => void;
    /** Jump the slideshow to this image; offered on member cards only. */
    onShow?: (path: string) => void;
  };

  export type CustomLayouts = {
    has: (path: string) => boolean;
    onReset: (path: string) => void;
  };

  type Props = {
    entries: LibraryEntry[];
    /** Add this entry to the canvas as a new layer (one image or many — same
     *  action). Also fired on double-click. */
    onApply: (entry: LibraryEntry) => void;
    onPin: (monitorId: string, path: string) => void;
    selection?: SlideshowSelection | null;
    customLayouts?: CustomLayouts | null;
  };
  let { entries, onApply, onPin, selection = null, customLayouts = null }: Props = $props();

  let scrollEl: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportH = $state(0);
  let viewportW = $state(0);
  let pinFor = $state<string | null>(null);

  const TILE_MIN_PX = 220;
  const ROW_GAP_PX = 12;
  const ROW_HEIGHT_PX = 178;

  const cols = $derived(
    Math.max(1, Math.floor((viewportW + ROW_GAP_PX) / (TILE_MIN_PX + ROW_GAP_PX))),
  );
  const totalRows = $derived(Math.ceil(entries.length / cols));
  const totalH = $derived(totalRows * (ROW_HEIGHT_PX + ROW_GAP_PX));
  const startRow = $derived(Math.max(0, Math.floor(scrollTop / (ROW_HEIGHT_PX + ROW_GAP_PX)) - 1));
  const endRow = $derived(
    Math.min(totalRows, Math.ceil((scrollTop + viewportH) / (ROW_HEIGHT_PX + ROW_GAP_PX)) + 1),
  );
  const visibleRange = $derived(
    entries.slice(startRow * cols, endRow * cols).map((entry, i) => {
      const idx = startRow * cols + i;
      return { entry, row: Math.floor(idx / cols), col: idx % cols };
    }),
  );

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

  $effect(() => {
    if (!scrollEl) return;
    const ro = new ResizeObserver(() => measure());
    ro.observe(scrollEl);
    measure();
    return () => ro.disconnect();
  });

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

<div bind:this={scrollEl} class="scroll" style:flex="1" style:padding="14px" onscroll={onScroll}>
  <div class="mono" style:font-size="11px" style:color="var(--text-3)" style:margin-bottom="10px">
    {entries.length} of {libraryStore.entries.length} images
  </div>
  {#if entries.length === 0}
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
        {@const m = selection ? selection.membershipOf(item.entry.path) : null}
        {@const hasCustom = customLayouts ? customLayouts.has(item.entry.path) : false}
        <div
          class="lib-card absolute"
          style:top="{item.row * (ROW_HEIGHT_PX + ROW_GAP_PX)}px"
          style:left="calc({(item.col * 100) / cols}% + {ROW_GAP_PX / 2}px)"
          style:width="calc({100 / cols}% - {ROW_GAP_PX}px)"
          style:height="{ROW_HEIGHT_PX}px"
          class:selected={m === 'image'}
          draggable="true"
          ondragstart={(ev) => onDragStart(ev, item.entry)}
          ondblclick={() => {
            // In selection mode the single click already toggled — a second
            // toggle here would make every double-click fire three times.
            if (!selection) onApply(item.entry);
          }}
          onclick={() => selection?.onToggle(item.entry.path)}
          onkeydown={(ev) => {
            if (selection && (ev.key === 'Enter' || ev.key === ' ')) {
              ev.preventDefault();
              selection.onToggle(item.entry.path);
            }
          }}
          title={item.entry.path}
          role="button"
          tabindex="0"
        >
          <div style:flex="1" style:min-height="0" style:position="relative">
            <LibraryThumb path={item.entry.path} alt={item.entry.path} />
            {#if selection}
              {#if m === 'image'}
                <div class="member member-image"><Icon name="check" size={10} /> in slideshow</div>
              {:else if m === 'folder'}
                <div class="member member-folder">
                  <Icon name="folder" size={10} /> via folder
                </div>
              {/if}
            {/if}
            {#if hasCustom}
              <div
                class="member member-custom"
                style:top={selection && m !== null ? '28px' : '6px'}
                title="This image has its own canvas layout in the slideshow"
              >
                <Icon name="layout" size={10} /> custom layout
              </div>
            {/if}
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
            {#if selection}
              <div class="flex" style:gap="4px" style:margin-top="8px">
                {#if m !== null && selection.onShow}
                  <button
                    class="btn primary sm"
                    style:flex="1"
                    style:font-size="10px"
                    title="Show this image on the canvas and desktop now"
                    onclick={(ev) => {
                      ev.stopPropagation();
                      selection.onShow?.(item.entry.path);
                    }}
                  >
                    Set
                  </button>
                {/if}
                <button
                  class="btn sm"
                  class:primary={m === null}
                  style:flex="1"
                  style:font-size="10px"
                  disabled={m === 'folder'}
                  title={m === 'folder'
                    ? 'Included via a folder source — remove the folder to exclude it'
                    : undefined}
                  onclick={(ev) => {
                    ev.stopPropagation();
                    selection.onToggle(item.entry.path);
                  }}
                >
                  {m === 'image' ? 'Remove' : m === 'folder' ? 'Via folder' : '+ Add'}
                </button>
              </div>
            {:else}
              <div class="flex" style:gap="4px" style:margin-top="8px">
                <button
                  class="btn primary sm"
                  style:flex="1"
                  style:font-size="10px"
                  title="Add this image to the canvas as a new layer"
                  onclick={(ev) => {
                    ev.stopPropagation();
                    onApply(item.entry);
                  }}
                >
                  + Add
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
            {/if}
            {#if hasCustom && customLayouts}
              <button
                class="btn sm"
                style:width="100%"
                style:margin-top="4px"
                style:font-size="10px"
                title="Remove this image's custom layout (back to the profile layout)"
                onclick={(ev) => {
                  ev.stopPropagation();
                  customLayouts.onReset(item.entry.path);
                }}
              >
                <Icon name="reset" size={10} /> Reset custom layout
              </button>
            {/if}
            {#if pinFor === item.entry.path}
              <LibraryPinMenu
                onPin={(monitorId) => {
                  onPin(monitorId, item.entry.path);
                  pinFor = null;
                }}
              />
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
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
  .lib-card.selected {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .member {
    position: absolute;
    top: 6px;
    left: 6px;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
  }
  .member-image {
    background: var(--accent);
    color: oklch(0.16 0.01 250);
  }
  .member-folder {
    background: oklch(0 0 0 / 0.55);
    color: oklch(1 0 0 / 0.85);
  }
  .member-custom {
    background: color-mix(in oklab, var(--accent) 35%, oklch(0 0 0 / 0.6));
    color: oklch(1 0 0 / 0.9);
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
</style>
