<script lang="ts">
  // Profile editor (PLAN §4a.4). Sits beside the canvas; its state lives in
  // `profileStore.draft`. Bezel sliders and image picker drive the canvas
  // through props on the parent.

  import { profileStore } from '$lib/stores/profile.svelte';
  import { isPerMonitorBody, isSpanBody, type FitMode } from '$lib/types/profile';
  import SpanSourceEditor from './profile/SpanSourceEditor.svelte';
  import ScheduleEditor from './profile/ScheduleEditor.svelte';

  const fitModes: FitMode[] = ['fill', 'fit', 'stretch', 'center'];

  function setBodyType(kind: 'span' | 'per_monitor') {
    profileStore.patchDraft((d) => {
      if (kind === 'span' && d.body.type !== 'span') {
        d.body = {
          type: 'span',
          source: { type: 'single', path: '' },
          fit: 'fill',
          offset: [0, 0],
        };
      } else if (kind === 'per_monitor' && d.body.type !== 'per_monitor') {
        d.body = { type: 'per_monitor', assignments: [], fit: 'fill' };
      }
    });
  }
</script>

<div class="flex h-full flex-col gap-3 overflow-y-auto p-1 text-sm">
  {#if !profileStore.draft}
    <div class="flex flex-col items-start gap-2 text-xs text-slate-500">
      <p>Pick a profile from the list, or create a new one to start editing.</p>
      <button
        type="button"
        class="rounded border border-slate-700 px-2 py-1 text-xs hover:bg-slate-800"
        onclick={() => profileStore.newProfile()}
      >
        New profile
      </button>
    </div>
  {:else}
    {@const draft = profileStore.draft}

    <header class="flex items-center justify-between gap-2">
      <div class="flex items-center gap-2">
        <input
          class="w-44 rounded border border-slate-700 bg-slate-900/40 px-2 py-1 font-mono text-sm text-slate-100"
          aria-label="Profile name"
          value={draft.name}
          oninput={(e) =>
            profileStore.patchDraft((d) => {
              d.name = (e.currentTarget as HTMLInputElement).value;
            })}
        />
        {#if profileStore.dirty}
          <span class="text-[10px] uppercase text-amber-400">unsaved</span>
        {/if}
      </div>
      <div class="flex gap-1">
        <button
          type="button"
          class="rounded border border-slate-700 px-2 py-1 text-xs hover:bg-slate-800 disabled:opacity-50"
          onclick={() => profileStore.revertDraft()}
          disabled={!profileStore.dirty || profileStore.saving}
        >
          Revert
        </button>
        <button
          type="button"
          class="rounded bg-accent/80 px-2 py-1 text-xs font-semibold text-slate-900 hover:bg-accent disabled:opacity-60"
          onclick={() => profileStore.save()}
          disabled={!profileStore.dirty || profileStore.saving || !draft.name.trim()}
        >
          {profileStore.saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    </header>

    <fieldset class="flex flex-col gap-2 rounded border border-slate-800 p-2">
      <legend class="px-1 text-[10px] uppercase tracking-wide text-slate-400">Body</legend>
      <div class="flex gap-1 text-xs">
        <button
          type="button"
          class="flex-1 rounded border px-2 py-1"
          class:border-accent={draft.body.type === 'span'}
          class:bg-accent={draft.body.type === 'span'}
          class:text-slate-900={draft.body.type === 'span'}
          class:border-slate-700={draft.body.type !== 'span'}
          onclick={() => setBodyType('span')}
        >
          Span
        </button>
        <button
          type="button"
          class="flex-1 rounded border px-2 py-1"
          class:border-accent={draft.body.type === 'per_monitor'}
          class:bg-accent={draft.body.type === 'per_monitor'}
          class:text-slate-900={draft.body.type === 'per_monitor'}
          class:border-slate-700={draft.body.type !== 'per_monitor'}
          onclick={() => setBodyType('per_monitor')}
        >
          Per monitor
        </button>
      </div>
    </fieldset>

    {#if isSpanBody(draft.body)}
      {@const span = draft.body}
      <SpanSourceEditor source={span.source} />
      <fieldset class="flex flex-col gap-2 rounded border border-slate-800 p-2">
        <legend class="px-1 text-[10px] uppercase tracking-wide text-slate-400">Fit</legend>
        <div class="flex gap-1 text-xs">
          {#each fitModes as f (f)}
            <button
              type="button"
              class="flex-1 rounded border px-2 py-1"
              class:border-accent={span.fit === f}
              class:border-slate-700={span.fit !== f}
              onclick={() =>
                profileStore.patchDraft((d) => {
                  if (isSpanBody(d.body)) d.body.fit = f;
                })}
            >
              {f}
            </button>
          {/each}
        </div>
      </fieldset>
    {:else if isPerMonitorBody(draft.body)}
      {@const pm = draft.body}
      <fieldset class="flex flex-col gap-2 rounded border border-slate-800 p-2">
        <legend class="px-1 text-[10px] uppercase tracking-wide text-slate-400">
          Per-monitor pins
        </legend>
        {#if pm.assignments.length === 0}
          <p class="text-xs text-slate-500">No monitor pins yet.</p>
        {:else}
          <ul class="flex flex-col gap-1">
            {#each pm.assignments as a, i (a.monitor.stable_id || a.monitor.name)}
              <li class="flex items-center gap-2 text-xs">
                <span class="w-20 truncate font-mono text-slate-300">{a.monitor.name}</span>
                <span class="flex-1 truncate font-mono text-slate-500">{a.path}</span>
                <button
                  type="button"
                  class="rounded border border-slate-700 px-1.5 py-0.5 text-[10px] hover:bg-slate-800"
                  onclick={() =>
                    profileStore.patchDraft((d) => {
                      if (isPerMonitorBody(d.body)) d.body.assignments.splice(i, 1);
                    })}
                >
                  Remove
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </fieldset>
    {/if}

    <fieldset class="flex flex-col gap-2 rounded border border-slate-800 p-2">
      <legend class="px-1 text-[10px] uppercase tracking-wide text-slate-400">Bezels (mm)</legend>
      <label class="flex items-center gap-2 text-xs text-slate-300">
        <span class="w-20 shrink-0">Horizontal</span>
        <input
          type="range"
          min="0"
          max="40"
          step="0.5"
          class="flex-1 accent-accent"
          value={draft.bezels.horizontal_mm}
          oninput={(e) =>
            profileStore.patchDraft((d) => {
              d.bezels.horizontal_mm = Number((e.currentTarget as HTMLInputElement).value);
            })}
        />
        <span class="w-12 text-right font-mono text-slate-200">
          {draft.bezels.horizontal_mm.toFixed(1)}
        </span>
      </label>
      <label class="flex items-center gap-2 text-xs text-slate-300">
        <span class="w-20 shrink-0">Vertical</span>
        <input
          type="range"
          min="0"
          max="40"
          step="0.5"
          class="flex-1 accent-accent"
          value={draft.bezels.vertical_mm}
          oninput={(e) =>
            profileStore.patchDraft((d) => {
              d.bezels.vertical_mm = Number((e.currentTarget as HTMLInputElement).value);
            })}
        />
        <span class="w-12 text-right font-mono text-slate-200">
          {draft.bezels.vertical_mm.toFixed(1)}
        </span>
      </label>
    </fieldset>

    <ScheduleEditor schedule={draft.schedule} profileName={draft.name} />
  {/if}
</div>
