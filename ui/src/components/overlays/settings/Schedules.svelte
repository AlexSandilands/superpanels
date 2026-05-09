<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from '$lib/api';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import type { Schedule } from '$lib/types/Schedule';
  import type { Trigger } from '$lib/types/Trigger';
  import type { LatLong } from '$lib/types/LatLong';
  import SectionHeader from './SectionHeader.svelte';

  let schedules = $state<Schedule[]>([]);
  let paused = $state(false);
  let location = $state<LatLong | null>(null);
  let loading = $state(false);

  let editing = $state<{ index: number | null; rule: Schedule } | null>(null);

  async function refresh() {
    loading = true;
    try {
      const r = await api.listSchedules();
      schedules = r.schedules;
      paused = r.paused;
      location = r.location;
    } catch (err) {
      toast.error('Failed to load schedules', errorMessage(err));
    } finally {
      loading = false;
    }
  }

  function describe(t: Trigger): string {
    if (t.type === 'daily')
      return `Daily at ${String(t.hour).padStart(2, '0')}:${String(t.minute).padStart(2, '0')}`;
    if (t.type === 'sunset') {
      const sign = t.offset_minutes >= 0 ? '+' : '−';
      return `Sunset ${sign}${Math.abs(t.offset_minutes)} min`;
    }
    if (t.type === 'sunrise') {
      const sign = t.offset_minutes >= 0 ? '+' : '−';
      return `Sunrise ${sign}${Math.abs(t.offset_minutes)} min`;
    }
    return `cron: ${t.expr}`;
  }

  function newRule() {
    const target = profileStore.profiles[0]?.name ?? '';
    editing = {
      index: null,
      rule: {
        display_name: null,
        profile: target,
        trigger: { type: 'daily', hour: 18, minute: 0 },
        enabled: true,
      },
    };
  }

  function editRule(i: number) {
    const rule = schedules[i];
    if (!rule) return;
    editing = { index: i, rule: structuredClone($state.snapshot(rule)) };
  }

  async function deleteRule(i: number) {
    const next = schedules.filter((_, j) => j !== i);
    try {
      await api.saveSchedules(next);
      schedules = next;
    } catch (err) {
      toast.error('Failed to delete rule', errorMessage(err));
    }
  }

  function detectConflict(rules: Schedule[]): { a: number; b: number } | null {
    const seen = new Map<string, number>();
    for (let i = 0; i < rules.length; i += 1) {
      const r = rules[i];
      if (!r || !r.enabled) continue;
      let key: string | null = null;
      if (r.trigger.type === 'daily') {
        key = `${r.trigger.hour}:${r.trigger.minute}`;
      } else if (r.trigger.type === 'sunset' || r.trigger.type === 'sunrise') {
        key = `${r.trigger.type}:${r.trigger.offset_minutes}`;
      } else if (r.trigger.type === 'cron') {
        key = `cron:${r.trigger.expr.trim()}`;
      }
      if (!key) continue;
      const prev = seen.get(key);
      if (prev !== undefined) return { a: prev, b: i };
      seen.set(key, i);
    }
    return null;
  }

  let editConflict = $derived(
    editing
      ? (() => {
          const next = [...schedules];
          if (editing.index === null) next.push(editing.rule);
          else next[editing.index] = editing.rule;
          return detectConflict(next);
        })()
      : null,
  );

  async function saveEdit() {
    if (!editing) return;
    if (editConflict) {
      toast.error(
        'Schedule conflict',
        `rules ${editConflict.a + 1} and ${editConflict.b + 1} fire at the same minute`,
      );
      return;
    }
    const next = [...schedules];
    if (editing.index === null) next.push(editing.rule);
    else next[editing.index] = editing.rule;
    try {
      await api.saveSchedules(next);
      schedules = next;
      editing = null;
    } catch (err) {
      toast.error('Failed to save rule', errorMessage(err));
    }
  }

  async function togglePaused() {
    try {
      const r = await api.setSchedulesPaused(!paused);
      paused = r.paused;
    } catch (err) {
      toast.error('Failed to toggle pause', errorMessage(err));
    }
  }

  onMount(() => {
    void refresh();
  });
</script>

<SectionHeader title="Schedules" sub="Time-of-day triggers that switch the active profile." />

<div class="row">
  <label class="toggle">
    <input type="checkbox" checked={paused} onchange={() => void togglePaused()} />
    <span>Pause all schedules</span>
  </label>
  <button class="btn sm" onclick={newRule} disabled={loading}>+ New rule</button>
</div>

{#if schedules.length === 0}
  <div class="empty">No rules yet. Click "+ New rule" to add one.</div>
{:else}
  {#each schedules as r, i (i)}
    <div class="rule">
      <label class="toggle">
        <input
          type="checkbox"
          checked={r.enabled}
          onchange={async () => {
            const next = schedules.map((rr, j) => (j === i ? { ...rr, enabled: !rr.enabled } : rr));
            try {
              await api.saveSchedules(next);
              schedules = next;
            } catch (err) {
              toast.error('Failed to toggle rule', errorMessage(err));
            }
          }}
        />
      </label>
      <div style:flex="1">
        <div class="title">{describe(r.trigger)} → {r.profile}</div>
        {#if r.display_name}<div class="sub">{r.display_name}</div>{/if}
      </div>
      <button class="btn sm" onclick={() => editRule(i)}>Edit</button>
      <button class="btn sm danger" onclick={() => void deleteRule(i)}>Delete</button>
    </div>
  {/each}
{/if}

{#if editing}
  <div class="editor">
    <h4>{editing.index === null ? 'New schedule rule' : 'Edit schedule rule'}</h4>
    <div class="grid">
      <label
        >Profile
        <select bind:value={editing.rule.profile}>
          {#each profileStore.profiles as p (p.name)}<option value={p.name}>{p.name}</option>{/each}
        </select>
      </label>
      <label
        >Trigger
        <select
          value={editing.rule.trigger.type}
          onchange={(e) => {
            const cur = editing;
            if (!cur) return;
            const v = (e.target as HTMLSelectElement).value;
            if (v === 'daily') cur.rule.trigger = { type: 'daily', hour: 18, minute: 0 };
            else if (v === 'sunset') cur.rule.trigger = { type: 'sunset', offset_minutes: 0 };
            else if (v === 'sunrise') cur.rule.trigger = { type: 'sunrise', offset_minutes: 0 };
            else cur.rule.trigger = { type: 'cron', expr: '0 0 * * * *' };
          }}
        >
          <option value="daily">Daily (HH:MM)</option>
          <option value="sunset">Sunset ± offset</option>
          <option value="sunrise">Sunrise ± offset</option>
          <option value="cron">Cron expression</option>
        </select>
      </label>
      {#if editing.rule.trigger.type === 'daily'}
        <label
          >Hour
          <input type="number" min="0" max="23" bind:value={editing.rule.trigger.hour} />
        </label>
        <label
          >Minute
          <input type="number" min="0" max="59" bind:value={editing.rule.trigger.minute} />
        </label>
      {:else if editing.rule.trigger.type === 'sunset' || editing.rule.trigger.type === 'sunrise'}
        <label
          >Offset (min)
          <input type="number" bind:value={editing.rule.trigger.offset_minutes} />
        </label>
      {:else if editing.rule.trigger.type === 'cron'}
        <label class="full"
          >Cron expression (6-field, space-separated)
          <input type="text" bind:value={editing.rule.trigger.expr} />
        </label>
      {/if}
      <label class="full"
        >Display name (optional)
        <input type="text" bind:value={editing.rule.display_name} />
      </label>
    </div>
    {#if editConflict}
      <div class="error">
        Conflict: rules {editConflict.a + 1} and {editConflict.b + 1} fire at the same minute.
      </div>
    {/if}
    <div class="actions">
      <button class="btn sm" onclick={() => (editing = null)}>Cancel</button>
      <button class="btn sm primary" onclick={() => void saveEdit()} disabled={!!editConflict}
        >Save</button
      >
    </div>
  </div>
{/if}

<div class="latlon">
  <h4>Location (for sunset / sunrise)</h4>
  {#if location}
    <div class="mono">lat={location.lat}, lon={location.lon}</div>
  {:else}
    <div class="hint">Not configured. Required for sunset / sunrise rules.</div>
  {/if}
  <p class="hint">
    Edit the top-level <code class="mono">location</code> in <code class="mono">config.toml</code>.
  </p>
</div>

<style>
  .row {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 8px 0;
  }
  .rule {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .title {
    font-size: 13px;
    font-weight: 500;
  }
  .sub {
    font-size: 11px;
    color: var(--text-3);
    margin-top: 2px;
  }
  .empty {
    padding: 14px 0;
    font-size: 12px;
    color: var(--text-3);
  }
  .toggle {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
  }
  .editor {
    margin-top: 14px;
    padding: 14px;
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .editor h4 {
    margin: 0 0 10px;
    font-size: 13px;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .grid label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
  }
  .grid label.full {
    grid-column: 1 / -1;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
  .error {
    color: var(--err);
    font-size: 12px;
    margin-top: 8px;
  }
  .latlon {
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  .latlon h4 {
    margin: 0 0 6px;
    font-size: 13px;
  }
  .hint {
    font-size: 11px;
    color: var(--text-3);
  }
  input,
  select {
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 4px 6px;
  }
</style>
