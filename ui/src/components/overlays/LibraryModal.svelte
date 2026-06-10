<script lang="ts">
  import { onMount } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import type { LibraryEntry } from '$lib/api';
  import { libraryStore } from '$lib/stores/library.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import {
    addFolder,
    addImage,
    membership,
    membershipLookup,
    removeImage,
    removeSourceAt,
    sourceLabel,
  } from '$lib/slideshow-set';
  import type { ImageSet } from '$lib/types/ImageSet';
  import Backdrop from '../widgets/Backdrop.svelte';
  import StepperInput from '../widgets/StepperInput.svelte';
  import Icon from '../widgets/Icon.svelte';
  import LibraryGrid from './LibraryGrid.svelte';

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
    /** Slideshow profile whose image set can be edited from here, if any. */
    slideshowTarget?: { name: string; images: ImageSet } | null;
    onUpdateSlideshow?: (images: ImageSet) => void;
  };
  let {
    onClose,
    onApplyAsSpan,
    onPinToMonitor,
    slideshowTarget = null,
    onUpdateSlideshow,
  }: Props = $props();

  let searchEl: HTMLInputElement | undefined = $state();
  // Editing the image set is the primary action while a slideshow profile is
  // selected, so the mode follows the target (which can settle a tick after
  // mount) until the user explicitly toggles the chip.
  let selectModeOverride = $state<boolean | null>(null);
  const selectMode = $derived(selectModeOverride ?? slideshowTarget !== null);

  const visible = $derived(libraryStore.visible);

  onMount(() => {
    if (libraryStore.entries.length === 0 && !libraryStore.loading) {
      void libraryStore.refresh();
    }
    if (searchEl) searchEl.focus();
  });

  async function pickFolder() {
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked === 'string') await libraryStore.addRoot(picked);
  }

  async function pickSlideshowFolder() {
    if (!slideshowTarget || !onUpdateSlideshow) return;
    const picked = await open({ directory: true, multiple: false });
    if (typeof picked !== 'string') return;
    const next = addFolder(slideshowTarget.images, picked, true);
    if (next === slideshowTarget.images) {
      toast.info('Already a source', picked);
      return;
    }
    onUpdateSlideshow(next);
  }

  function toggleMembership(path: string) {
    if (!slideshowTarget || !onUpdateSlideshow) return;
    const m = membership(slideshowTarget.images, path);
    if (m === 'image') {
      onUpdateSlideshow(removeImage(slideshowTarget.images, path));
    } else if (m === null) {
      onUpdateSlideshow(addImage(slideshowTarget.images, path));
    } else {
      toast.info('Included via folder', 'remove the folder source to exclude it');
    }
  }

  const selection = $derived(
    selectMode && slideshowTarget && onUpdateSlideshow
      ? {
          membershipOf: membershipLookup(slideshowTarget.images),
          onToggle: toggleMembership,
        }
      : null,
  );

  function applyEntry(entry: LibraryEntry) {
    onApplyAsSpan(entry.path);
    onClose();
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
      {#if slideshowTarget}
        <button
          class="chip"
          class:active={selectMode}
          title={`Add images to '${slideshowTarget.name}'`}
          onclick={() => (selectModeOverride = !selectMode)}
        >
          <Icon name="stack" size={11} /> add to slideshow
        </button>
      {/if}
      <div style:flex="1"></div>
      {#if selectMode && slideshowTarget}
        <div class="mono" style:font-size="11px" style:color="var(--text-3)">
          editing ‘{slideshowTarget.name}’
        </div>
      {/if}
      <button class="btn ghost icon" onclick={onClose} aria-label="Close">×</button>
    </div>

    {#if libraryStore.roots.length === 0 && !libraryStore.loading}
      <div class="empty">
        <div class="empty-title">No library folders yet</div>
        <p>
          Add a folder of images to start building your library. Subfolders are scanned
          automatically.
        </p>
        <button
          class="btn primary"
          onclick={() => void pickFolder()}
          disabled={libraryStore.busyRoots}
        >
          + Add your first folder
        </button>
      </div>
    {:else}
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

          {#if selectMode && slideshowTarget && onUpdateSlideshow}
            <div class="section-label" style:margin-top="18px">Slideshow sources</div>
            {#if slideshowTarget.images.sources.length === 0}
              <p class="src-hint">Nothing yet — click images on the right or add a whole folder.</p>
            {/if}
            {#each slideshowTarget.images.sources as source, i (`${source.type}:${source.path}`)}
              <div class="root-row" title={source.path}>
                <Icon name={source.type === 'folder' ? 'folder' : 'cover'} />
                <div style:flex="1" style:min-width="0">
                  <div
                    class="mono"
                    style:font-size="11px"
                    style:overflow="hidden"
                    style:text-overflow="ellipsis"
                    style:white-space="nowrap"
                    style:color="var(--text-2)"
                  >
                    {sourceLabel(source)}
                  </div>
                </div>
                <button
                  class="btn ghost sm"
                  style:padding="0 6px"
                  title="Remove from slideshow"
                  onclick={() => onUpdateSlideshow(removeSourceAt(slideshowTarget.images, i))}
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
              onclick={() => void pickSlideshowFolder()}
            >
              <Icon name="plus" size={11} /> Add folder as source
            </button>
          {/if}

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

        <LibraryGrid entries={visible} onApply={applyEntry} onPin={onPinToMonitor} {selection} />
      </div>
    {/if}
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
  .src-hint {
    margin: 0 0 6px;
    font-size: 11px;
    color: var(--text-3);
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
  .empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-direction: column;
    gap: 12px;
    padding: 40px;
    text-align: center;
  }
  .empty-title {
    font-size: 14px;
    font-weight: 600;
  }
  .empty p {
    font-size: 12px;
    color: var(--text-3);
    max-width: 360px;
    margin: 0;
  }
</style>
