<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from '$lib/api';
  import type { Profile, Monitor } from '$lib/api';
  import type { ProfileColour } from '$lib/types/ProfileColour';
  import type { ProfileValidity } from '$lib/types/ProfileValidity';
  import type { DisableReason } from '$lib/types/DisableReason';
  import { switchAndApply } from '$lib/actions';
  import { toast } from '$lib/stores/toast.svelte';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { profileColourCss } from '$lib/profile-colours';
  import { topologyRects } from '$lib/profile-topology';
  import { loadSourceImage, type SourceImage } from '$lib/library/source-image';
  import Backdrop from '../widgets/Backdrop.svelte';
  import ConfirmDialog from '../widgets/ConfirmDialog.svelte';
  import SaveProfileDialog from './SaveProfileDialog.svelte';
  import MonitorMini from './profiles/MonitorMini.svelte';
  import ColorPopover from './profiles/ColorPopover.svelte';
  import ProfilePreview from './profiles/ProfilePreview.svelte';

  type Props = {
    onClose: () => void;
    onCreateFromCanvas?: (
      name: string,
      colour: ProfileColour,
      description: string | null,
    ) => Promise<void> | void;
  };
  let { onClose, onCreateFromCanvas }: Props = $props();

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
  let colorPopover = $state(false);
  let previewWidth = $state(560);

  function profileImagePath(p: Profile): string | null {
    if (p.body.type !== 'span') return null;
    const src = p.body.source;
    if (src.type !== 'single') return null;
    const path = src.path.trim();
    if (!path) return null;
    if (!path.startsWith('/')) return null;
    return path;
  }

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
    if (b.type === 'span') {
      const src = b.source;
      if (src.type === 'single') return src.path || '— no image —';
      if (src.images.type === 'folder') return src.images.path;
      return `Playlist (${src.images.paths.length} images)`;
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
  let detailValidity = $derived(detail ? validity[detail.name] : undefined);
  let detailRects = $derived(detail ? topologyRects(detail.monitor_state, detectedMonitors) : []);
  let detailDisabled = $derived(isDisabled(detailValidity));
  let detailMismatch = $derived(hasTopologyMismatch(detailValidity));
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

  async function updateColour(p: Profile, colour: ProfileColour) {
    try {
      await api.saveProfile({ ...p, colour, updated_at: new Date().toISOString() });
      void refresh();
    } catch (err) {
      toast.error('Colour update failed', errorMessage(err));
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

  async function handleCreate(name: string, colour: ProfileColour, description: string | null) {
    showSave = false;
    if (!onCreateFromCanvas) return;
    await onCreateFromCanvas(name, colour, description);
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
                        ? `center/cover no-repeat url("${thumb.url}"), ${profileColourCss(p.colour)}`
                        : profileColourCss(p.colour)}
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
                        <span class="dot-swatch" style:background={profileColourCss(p.colour)}
                        ></span>
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
                width={previewWidth}
                height={240}
                background={profileColourCss(detail.colour)}
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
              <div class="swatch-wrap">
                <button
                  class="swatch-button"
                  title="Change colour"
                  style:background={profileColourCss(detail.colour)}
                  onclick={() => (colorPopover = !colorPopover)}
                  aria-label="Change colour"
                ></button>
                {#if colorPopover}
                  <ColorPopover
                    value={detail.colour}
                    onPick={(c) => {
                      colorPopover = false;
                      void updateColour(detail, c);
                    }}
                    onClose={() => (colorPopover = false)}
                  />
                {/if}
              </div>
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
                <button class="btn sm ghost" disabled title="Coming soon">Reveal</button>
              </span>
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
              {#if detailDisabled}
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
    onConfirm={(n, c, d) => void handleCreate(n, c, d)}
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
      confirmDel = null;
      void deleteProfile(target);
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
  .dot-swatch {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    border: 1px solid var(--line);
    flex-shrink: 0;
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
  .swatch-wrap {
    position: relative;
  }
  .swatch-button {
    width: 36px;
    height: 24px;
    border-radius: 4px;
    border: 1px solid var(--line);
    cursor: default;
    padding: 0;
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
