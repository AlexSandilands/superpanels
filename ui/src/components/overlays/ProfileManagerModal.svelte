<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from '$lib/api';
  import type { Profile, Monitor } from '$lib/api';
  import type { ProfileValidity } from '$lib/types/ProfileValidity';
  import type { DisableReason } from '$lib/types/DisableReason';
  import { switchAndApply } from '$lib/actions';
  import { toast } from '$lib/stores/toast.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import {
    isSlideshowBody,
    isStandardBody,
    type ProfileKind,
    type SlideshowSort,
    type SlideshowSource,
  } from '$lib/types/profile-helpers';
  import { folderCount, imageCount } from '$lib/slideshow-set';
  import { topologyRects } from '$lib/profile-topology';
  import { loadSourceImage, type SourceImage } from '$lib/library/source-image';
  import Backdrop from '../widgets/Backdrop.svelte';
  import ConfirmDialog from '../widgets/ConfirmDialog.svelte';
  import SaveProfileDialog from './SaveProfileDialog.svelte';
  import MonitorMini from './profiles/MonitorMini.svelte';
  import ProfilePreview, { type PreviewLayer } from './profiles/ProfilePreview.svelte';

  const PROFILE_BG = 'oklch(0.22 0 0)';

  type Props = {
    onClose: () => void;
    onCreateFromCanvas?: (
      name: string,
      description: string | null,
      kind: ProfileKind,
    ) => Promise<void> | void;
    /** Select the profile and open the library's slideshow editor. */
    onEditSlideshow?: (p: Profile) => void;
  };
  let { onClose, onCreateFromCanvas, onEditSlideshow }: Props = $props();

  let profiles = $state<Profile[]>([]);
  let validity = $state<Record<string, ProfileValidity>>({});
  let detectedMonitors = $state<Monitor[]>([]);
  let thumbnails = $state<Record<string, SourceImage>>({});
  let search = $state('');
  let selectedName = $state<string | null>(null);
  let loading = $state(false);
  let showSave = $state(false);
  let confirmDel = $state<Profile | null>(null);
  let editingName = $state(false);
  let previewWidth = $state(560);

  function profileImagePath(p: Profile): string | null {
    if (p.body.type === 'standard') {
      // Best-effort swatch: the top layer's image.
      const top = p.body.layers.at(-1);
      return top && top.path.startsWith('/') ? top.path : null;
    }
    if (p.body.type !== 'slideshow') return null;
    // The first hand-picked image. Folder sources would need a scan, which
    // isn't worth it for a list swatch.
    const img = p.body.source.images.sources.find((s) => s.type === 'image');
    return img && img.path.startsWith('/') ? img.path : null;
  }

  function slideshowOf(p: Profile): SlideshowSource | null {
    return isSlideshowBody(p.body) ? p.body.source : null;
  }

  function intervalLabel(secs: number): string {
    if (secs >= 3600 && secs % 3600 === 0) return `${secs / 3600} h`;
    if (secs >= 60) return `${Math.round(secs / 60)} min`;
    return `${secs} s`;
  }

  const sortLabels: Record<SlideshowSort, string> = {
    shuffle: 'Shuffle',
    alphabetical: 'A → Z',
    date_asc: 'Oldest first',
    date_desc: 'Newest first',
    last_shown_asc: 'Least recently shown',
  };

  async function loadThumbnail(p: Profile): Promise<void> {
    const path = profileImagePath(p);
    if (!path) return;
    try {
      thumbnails[p.name] = await loadSourceImage(path);
    } catch {
      // ignore — missing/unreadable images stay swatch-only
    }
  }

  async function refresh() {
    loading = true;
    try {
      const [r, monitors] = await Promise.all([api.listProfiles(), api.detectMonitors()]);
      profiles = r.profiles;
      validity = Object.fromEntries(r.validity.map((e) => [e.profile, e.validity]));
      detectedMonitors = monitors;
      if (selectedName && !profiles.some((p) => p.name === selectedName)) {
        selectedName = profiles[0]?.name ?? null;
      } else if (!selectedName) {
        selectedName = profileStore.activeName ?? profiles[0]?.name ?? null;
      }
      const next: Record<string, SourceImage> = {};
      for (const p of profiles) {
        const cached = thumbnails[p.name];
        if (cached) next[p.name] = cached;
      }
      thumbnails = next;
      void Promise.all(profiles.map((p) => loadThumbnail(p)));
    } catch (err) {
      toast.error('Failed to load profiles', errorMessage(err));
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void refresh();
  });

  function recencyText(ts: string | null | undefined): string {
    if (!ts) return 'never';
    const d = new Date(ts);
    const s = Math.max(0, (Date.now() - d.getTime()) / 1000);
    if (s < 30) return 'just now';
    if (s < 90) return '1m ago';
    if (s < 3600) return `${Math.round(s / 60)}m ago`;
    if (s < 3600 * 24) return `${Math.round(s / 3600)}h ago`;
    if (s < 3600 * 24 * 14) return `${Math.round(s / 86400)}d ago`;
    return `${Math.round(s / (86400 * 7))}w ago`;
  }

  function sourceLabel(p: Profile): string {
    const b = p.body;
    if (b.type === 'standard') {
      return `Standard (${b.layers.length} image${b.layers.length === 1 ? '' : 's'})`;
    }
    if (b.type === 'slideshow') {
      const src = b.source;
      if (src.images.sources.length === 0) return 'Slideshow (empty)';
      const folders = folderCount(src.images);
      const images = imageCount(src.images);
      const parts = [
        folders > 0 ? `${folders} folder${folders === 1 ? '' : 's'}` : null,
        images > 0 ? `${images} image${images === 1 ? '' : 's'}` : null,
      ].filter(Boolean);
      return `Slideshow (${parts.join(', ')})`;
    }
    return `Per-monitor (${b.assignments.length})`;
  }

  function disableReasonText(r: DisableReason): string {
    switch (r.kind) {
      case 'topology_mismatch':
        return 'authored for a different setup';
      case 'image_missing':
        return `image missing: ${r.path}`;
      case 'folder_missing_or_empty':
        return `folder missing or empty: ${r.path}`;
      case 'slideshow_empty':
        return 'slideshow has no images yet';
      case 'monitor_not_connected':
        return `monitor not connected (${r.monitor.name})`;
      case 'physical_size_missing':
        return `monitor size not set (${r.stable_id})`;
    }
  }

  function isDisabled(v: ProfileValidity | undefined): boolean {
    return !!v && v.kind === 'disabled';
  }

  function hasTopologyMismatch(v: ProfileValidity | undefined): boolean {
    return !!v && v.kind === 'disabled' && v.reasons.some((r) => r.kind === 'topology_mismatch');
  }

  function disabledSummary(v: ProfileValidity | undefined): string {
    if (!v || v.kind !== 'disabled') return '';
    const first = v.reasons[0];
    return first ? disableReasonText(first) : '';
  }

  let activeName = $derived(profileStore.activeName);

  let filtered = $derived(
    [...profiles]
      .filter(
        (p) =>
          !search ||
          p.name.toLowerCase().includes(search.toLowerCase()) ||
          (p.description ?? '').toLowerCase().includes(search.toLowerCase()),
      )
      .sort((a, b) => {
        const ta = a.last_applied_at ? new Date(a.last_applied_at).getTime() : 0;
        const tb = b.last_applied_at ? new Date(b.last_applied_at).getTime() : 0;
        return tb - ta;
      }),
  );
  let detail = $derived(profiles.find((p) => p.name === selectedName) ?? null);
  let detailSlideshow = $derived(detail ? slideshowOf(detail) : null);

  // Composite preview for the selected standard profile: each layer at its
  // mm-space rect. Rects are known immediately; images resolve async.
  let detailLayers = $state<PreviewLayer[] | null>(null);
  $effect(() => {
    const d = detail;
    if (!d || !isStandardBody(d.body)) {
      detailLayers = null;
      return;
    }
    const layers = d.body.layers;
    detailLayers = layers.map((l) => ({ url: null, rect: l.image_rect_mm }));
    let cancelled = false;
    void Promise.all(
      layers.map((l) =>
        loadSourceImage(l.path)
          .then((img) => img.url)
          .catch(() => null),
      ),
    ).then((urls) => {
      if (cancelled) return;
      detailLayers = layers.map((l, i) => ({ url: urls[i] ?? null, rect: l.image_rect_mm }));
    });
    return () => {
      cancelled = true;
    };
  });
  let detailValidity = $derived(detail ? validity[detail.name] : undefined);
  let detailRects = $derived(detail ? topologyRects(detail.monitor_state, detectedMonitors) : []);
  let detailDisabled = $derived(isDisabled(detailValidity));
  let detailMismatch = $derived(hasTopologyMismatch(detailValidity));
  let detailNeedsImages = $derived(
    detailSlideshow !== null &&
      detailValidity?.kind === 'disabled' &&
      detailValidity.reasons.some(
        (r) => r.kind === 'slideshow_empty' || r.kind === 'folder_missing_or_empty',
      ),
  );
  let isActive = $derived(detail !== null && detail.name === activeName);

  async function applyProfile(p: Profile) {
    await switchAndApply(p);
    void profileStore.refresh();
    void refresh();
  }

  async function deleteProfile(p: Profile) {
    try {
      await api.deleteProfile(p.name);
      toast.success(`Deleted '${p.name}'`);
      void profileStore.refresh();
      void refresh();
    } catch (err) {
      toast.error('Delete failed', errorMessage(err));
    }
  }

  async function duplicateProfile(p: Profile) {
    const newName = `${p.name}-copy`;
    try {
      await api.duplicateProfile(p.name, newName);
      toast.success(`Duplicated as '${newName}'`);
      selectedName = newName;
      void refresh();
    } catch (err) {
      toast.error('Duplicate failed', errorMessage(err));
    }
  }

  async function commitRename(p: Profile, newName: string) {
    const trimmed = newName.trim();
    editingName = false;
    if (!trimmed || trimmed === p.name) return;
    if (profiles.some((x) => x.name === trimmed)) {
      toast.error('Rename failed', 'name already taken');
      return;
    }
    try {
      await api.renameProfile(p.name, trimmed);
      selectedName = trimmed;
      void refresh();
    } catch (err) {
      toast.error('Rename failed', errorMessage(err));
    }
  }

  async function commitDescription(p: Profile, value: string) {
    const next = value.trim() || null;
    if (next === (p.description ?? null)) return;
    try {
      await api.saveProfile({
        ...p,
        description: next,
        updated_at: new Date().toISOString(),
      });
      void refresh();
    } catch (err) {
      toast.error('Description update failed', errorMessage(err));
    }
  }

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function repair(_p: Profile): void {
    void _p;
    toast.info('Repair flow', 'reposition monitors / image on the canvas, then save.');
    onClose();
  }

  function openCreate() {
    if (!onCreateFromCanvas) {
      toast.error('Cannot create profile here', 'no canvas state available');
      return;
    }
    showSave = true;
  }

  async function handleCreate(name: string, description: string | null, kind: ProfileKind) {
    showSave = false;
    if (!onCreateFromCanvas) return;
    if (kind === 'slideshow') {
      // Creation drops into the library's slideshow editor — get the manager
      // out of the way so it isn't stacked over it.
      onClose();
      await onCreateFromCanvas(name, description, kind);
      return;
    }
    await onCreateFromCanvas(name, description, kind);
    selectedName = name;
    void refresh();
  }
</script>

<Backdrop {onClose}>
  <div class="manager panel">
    <header class="bar">
      <div class="title">Profiles</div>
      <span class="chip count">{profiles.length}</span>
      <div class="search">
        <svg width="11" height="11" viewBox="0 0 11 11" aria-hidden="true">
          <circle cx="4.5" cy="4.5" r="2.7" fill="none" stroke="currentColor" stroke-width="1.2"
          ></circle>
          <path d="M6.5 6.5L9 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"
          ></path>
        </svg>
        <input bind:value={search} placeholder="Search profiles…" />
      </div>
      <div class="spacer"></div>
      <button class="btn" onclick={openCreate}>+ New profile</button>
      <button class="btn" disabled title="Coming soon">Import…</button>
      <button class="btn ghost icon" onclick={onClose} aria-label="Close">×</button>
    </header>

    {#if profiles.length === 0 && !loading}
      <div class="empty">
        <div class="empty-title">No profiles yet</div>
        <p>
          A profile bundles an image, the way it's cropped, and your monitor arrangement. Make one
          to switch in a click.
        </p>
        <button class="btn primary" onclick={openCreate}>+ Create your first profile</button>
      </div>
    {:else}
      <div class="body">
        <aside class="rail">
          {#if loading && profiles.length === 0}
            <div class="empty-mini">Loading…</div>
          {:else if filtered.length === 0}
            <div class="empty-mini">No matches.</div>
          {:else}
            <ul>
              {#each filtered as p (p.name)}
                {@const v = validity[p.name]}
                {@const disabled = isDisabled(v)}
                {@const mismatch = hasTopologyMismatch(v)}
                {@const thumb = thumbnails[p.name]}
                <li class:selected={selectedName === p.name} class:disabled>
                  <button onclick={() => (selectedName = p.name)}>
                    <div
                      class="thumb"
                      class:has-image={!!thumb}
                      style:background={thumb
                        ? `center/cover no-repeat url("${thumb.url}"), ${PROFILE_BG}`
                        : PROFILE_BG}
                    >
                      <div class="thumb-mini">
                        <MonitorMini
                          rects={topologyRects(p.monitor_state, detectedMonitors)}
                          width={48}
                          height={28}
                          color="oklch(1 0 0 / 0.85)"
                        />
                      </div>
                    </div>
                    <div class="row-main">
                      <div class="row-name">
                        <span class="name" title={p.name}>{p.name}</span>
                        {#if p.name === activeName}
                          <span class="dot live" title="active"></span>
                        {/if}
                      </div>
                      <div class="row-source mono" title={sourceLabel(p)}>
                        {sourceLabel(p)}
                      </div>
                    </div>
                    <div class="row-meta">
                      {#if disabled}
                        <span class="warn-text" title={disabledSummary(v)}> ⚠ Disabled </span>
                      {:else}
                        <span class="recency">{recencyText(p.last_applied_at)}</span>
                      {/if}
                      {#if mismatch && !disabled}
                        <span class="mini-chip">other setup</span>
                      {/if}
                    </div>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </aside>

        {#if detail}
          {@const detailThumb = thumbnails[detail.name]}
          <main class="detail">
            <div class="preview-wrap" bind:clientWidth={previewWidth}>
              <ProfilePreview
                rects={detailRects}
                imageUrl={detailThumb?.url ?? null}
                naturalDims={detailThumb
                  ? { w: detailThumb.naturalW, h: detailThumb.naturalH }
                  : null}
                layers={detailLayers}
                width={previewWidth}
                height={240}
                background={PROFILE_BG}
                disabled={detailDisabled}
              />
              <div class="preview-source mono">{sourceLabel(detail)}</div>
              {#if detailDisabled}
                <div class="preview-disabled">
                  <span>⚠ Disabled · {disabledSummary(detailValidity)}</span>
                </div>
              {/if}
            </div>

            <div class="name-row">
              {#if editingName}
                <input
                  class="field ui name-input"
                  use:focusOnMount
                  value={detail.name}
                  onblur={(e) => void commitRename(detail, (e.target as HTMLInputElement).value)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter')
                      void commitRename(detail, (e.target as HTMLInputElement).value);
                    if (e.key === 'Escape') editingName = false;
                  }}
                />
              {:else}
                <button
                  class="name-display"
                  onclick={() => (editingName = true)}
                  title="Click to rename"
                >
                  {detail.name}
                </button>
              {/if}
              {#if detailSlideshow}
                <span class="chip">SLIDESHOW</span>
              {/if}
              {#if isActive}
                <span class="chip active">ACTIVE</span>
              {/if}
              {#if detailMismatch && !detailDisabled}
                <span class="chip warn">Different setup</span>
              {/if}
            </div>

            <textarea
              class="field ui description"
              placeholder="Description (optional)"
              value={detail.description ?? ''}
              onblur={(e) =>
                void commitDescription(detail, (e.target as HTMLTextAreaElement).value)}
            ></textarea>

            <div class="meta-grid">
              <span class="meta-label">Source</span>
              <span class="meta-value mono">
                {sourceLabel(detail)}
                {#if detailSlideshow && onEditSlideshow}
                  {@const editTarget = detail}
                  <button class="btn sm ghost" onclick={() => onEditSlideshow(editTarget)}>
                    Edit images…
                  </button>
                {/if}
              </span>
              {#if detailSlideshow}
                <span class="meta-label">Interval</span>
                <span class="meta-value mono">
                  every {intervalLabel(detailSlideshow.config.interval_secs)}
                </span>
                <span class="meta-label">Order</span>
                <span class="meta-value mono">{sortLabels[detailSlideshow.config.sort]}</span>
              {/if}
              <span class="meta-label">Topology</span>
              <span class="meta-value mono">
                {Object.keys(detail.monitor_state).length} monitors
                {#if detailMismatch}
                  <span class="meta-warn">· authored for a different setup</span>
                {/if}
              </span>
              <span class="meta-label">Last used</span>
              <span class="meta-value mono">{recencyText(detail.last_applied_at)}</span>
              <span class="meta-label">Created</span>
              <span class="meta-value mono">{recencyText(detail.created_at)}</span>
            </div>

            <div class="actions">
              {#if detailNeedsImages && onEditSlideshow}
                {@const target = detail}
                <button class="btn primary" onclick={() => onEditSlideshow(target)}>
                  Add images…
                </button>
              {:else if detailDisabled}
                <button class="btn primary" onclick={() => repair(detail)}> 🔧 Repair </button>
              {:else}
                <button
                  class="btn primary"
                  disabled={isActive}
                  onclick={() => void applyProfile(detail)}
                >
                  Apply
                </button>
              {/if}
              <button class="btn" onclick={() => void duplicateProfile(detail)}>Duplicate</button>
              <button class="btn" disabled title="Coming soon">Export</button>
              <div class="actions-spacer"></div>
              <button class="btn danger" onclick={() => (confirmDel = detail)}>Delete</button>
            </div>
          </main>
        {:else}
          <main class="detail-empty">
            <div>Select a profile from the list.</div>
          </main>
        {/if}
      </div>
    {/if}
  </div>
</Backdrop>

{#if showSave}
  <SaveProfileDialog
    existingNames={profiles.map((p) => p.name)}
    defaultName={`untitled-${profiles.length + 1}`}
    onCancel={() => (showSave = false)}
    onConfirm={(n, d, k) => void handleCreate(n, d, k)}
  />
{/if}

{#if confirmDel}
  {@const target = confirmDel}
  <ConfirmDialog
    title={`Delete "${target.name}"?`}
    body="This can't be undone. Schedules pointing at this profile will be flagged."
    danger
    confirmLabel="Delete"
    onCancel={() => (confirmDel = null)}
    onConfirm={() => {
      const t = target;
      confirmDel = null;
      void deleteProfile(t);
    }}
  />
{/if}

<style>
  .manager {
    width: min(1100px, 94vw);
    height: min(680px, 88vh);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-size: 14px;
    font-weight: 600;
  }
  .chip.count {
    font-size: 10px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 6px;
    height: 28px;
    padding: 0 10px;
    flex: 1;
    max-width: 320px;
    color: var(--text-3);
  }
  .search input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-size: 12px;
    min-width: 0;
  }
  .spacer {
    flex: 1;
  }

  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .rail {
    width: 380px;
    border-right: 1px solid var(--line);
    overflow-y: auto;
  }
  .rail ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .rail li {
    border-bottom: 1px solid var(--line);
  }
  .rail li.disabled {
    opacity: 0.55;
  }
  .rail li button {
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    padding: 10px 14px;
    display: grid;
    grid-template-columns: 56px 1fr auto;
    gap: 12px;
    align-items: center;
    color: inherit;
    cursor: default;
  }
  .rail li.selected button {
    background: color-mix(in oklab, var(--accent) 12%, transparent);
  }
  .rail li:not(.selected) button:hover {
    background: var(--panel-2);
  }
  .thumb {
    width: 56px;
    height: 36px;
    border-radius: 4px;
    border: 1px solid var(--line);
    position: relative;
    overflow: hidden;
    flex-shrink: 0;
  }
  .thumb-mini {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0.85;
  }
  .row-main {
    min-width: 0;
  }
  .row-name {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 2px;
  }
  .name {
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-source {
    font-size: 10px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-meta {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    flex-shrink: 0;
  }
  .recency {
    font-size: 10px;
    color: var(--text-3);
  }
  .warn-text {
    font-size: 10px;
    color: var(--warn);
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .mini-chip {
    font-size: 9px;
    color: var(--text-3);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 4px;
  }

  .detail {
    flex: 1;
    padding: 22px;
    overflow-y: auto;
    min-width: 0;
  }
  .detail-empty {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-3);
    font-size: 12px;
  }
  .preview-wrap {
    position: relative;
    border-radius: 8px;
    border: 1px solid var(--line);
    overflow: hidden;
    margin-bottom: 16px;
    width: 100%;
  }
  .preview-source {
    position: absolute;
    top: 10px;
    left: 12px;
    font-size: 10px;
    color: oklch(1 0 0 / 0.85);
    text-shadow: 0 1px 2px oklch(0 0 0 / 0.5);
    max-width: 70%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .preview-disabled {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: flex-end;
    padding: 12px;
    background: linear-gradient(180deg, transparent 60%, oklch(0 0 0 / 0.55));
    color: white;
    font-size: 12px;
  }

  .name-row {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }
  .name-input {
    flex: 1;
    font-size: 16px;
    height: 30px;
  }
  .name-display {
    flex: 1;
    text-align: left;
    font-size: 18px;
    font-weight: 600;
    background: transparent;
    border: none;
    color: var(--text);
    cursor: text;
    padding: 2px 4px;
    border-radius: 4px;
  }
  .name-display:hover {
    background: var(--panel-2);
  }
  .chip.warn {
    color: var(--warn);
    border-color: color-mix(in oklab, var(--warn) 40%, var(--line));
  }

  .description {
    width: 100%;
    min-height: 56px;
    padding: 10px;
    resize: vertical;
    font-size: 12px;
    line-height: 1.5;
    margin-bottom: 16px;
    box-sizing: border-box;
  }

  .meta-grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 14px;
    font-size: 12px;
    margin-bottom: 16px;
  }
  .meta-label {
    color: var(--text-3);
  }
  .meta-value {
    color: var(--text);
  }
  .meta-warn {
    color: var(--warn);
    margin-left: 8px;
  }

  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    border-top: 1px solid var(--line);
    padding-top: 14px;
    align-items: center;
  }
  .actions-spacer {
    flex: 1;
  }
  .btn.danger {
    color: var(--danger);
    border-color: color-mix(in oklab, var(--danger) 40%, var(--line));
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
    max-width: 340px;
    margin: 0;
  }
  .empty-mini {
    padding: 24px;
    text-align: center;
    color: var(--text-3);
    font-size: 12px;
  }
</style>
