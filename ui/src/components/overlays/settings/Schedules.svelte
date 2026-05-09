<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from '$lib/api';
  import { profileStore } from '$lib/stores/profile.svelte';
  import { toast } from '$lib/stores/toast.svelte';
  import type { Schedule } from '$lib/types/Schedule';
  import type { Trigger } from '$lib/types/Trigger';
  import Icon from '../../widgets/Icon.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';
  import Toggle from './Toggle.svelte';
  import ScheduleRow from './ScheduleRow.svelte';
  import ScheduleEditor from './ScheduleEditor.svelte';

  let schedules = $state<Schedule[]>([]);
  let paused = $state(false);
  let loading = $state(false);

  let editing = $state<{ index: number | null; rule: Schedule } | null>(null);

  async function refresh() {
    loading = true;
    try {
      const r = await api.listSchedules();
      schedules = r.schedules;
      paused = r.paused;
    } catch (err) {
      toast.error('Failed to load schedules', errorMessage(err));
    } finally {
      loading = false;
    }
  }

  let profileNames = $derived(profileStore.profiles.map((p) => p.name));

  function newRule() {
    const target = profileNames[0] ?? '';
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

  function triggerKey(t: Trigger): string {
    if (t.type === 'daily') return `${t.hour}:${t.minute}`;
    if (t.type === 'sunset' || t.type === 'sunrise') return `${t.type}:${t.offset_minutes}`;
    return `cron:${t.expr.trim()}`;
  }

  function detectConflict(rules: Schedule[]): { a: number; b: number } | null {
    const seen = new Map<string, number>();
    for (let i = 0; i < rules.length; i += 1) {
      const r = rules[i];
      if (!r || !r.enabled) continue;
      const key = triggerKey(r.trigger);
      const prev = seen.get(key);
      if (prev !== undefined) return { a: prev, b: i };
      seen.set(key, i);
    }
    return null;
  }

  let editConflict = $derived.by(() => {
    if (!editing) return null;
    const next = [...schedules];
    if (editing.index === null) next.push(editing.rule);
    else next[editing.index] = editing.rule;
    const c = detectConflict(next);
    if (!c) return null;
    const otherIdx = c.a === editing.index || c.a === next.length - 1 ? c.b : c.a;
    return next[otherIdx] ?? null;
  });

  async function saveEdit(updated: Schedule) {
    if (!editing) return;
    if (editConflict) return;
    const next = [...schedules];
    if (editing.index === null) next.push(updated);
    else next[editing.index] = updated;
    try {
      await api.saveSchedules(next);
      schedules = next;
      editing = null;
    } catch (err) {
      toast.error('Failed to save rule', errorMessage(err));
    }
  }

  async function togglePaused(v: boolean) {
    try {
      const r = await api.setSchedulesPaused(v);
      paused = r.paused;
    } catch (err) {
      toast.error('Failed to toggle pause', errorMessage(err));
    }
  }

  async function toggleRule(i: number, v: boolean) {
    const next = schedules.map((rr, j) => (j === i ? { ...rr, enabled: v } : rr));
    try {
      await api.saveSchedules(next);
      schedules = next;
    } catch (err) {
      toast.error('Failed to toggle rule', errorMessage(err));
    }
  }

  onMount(() => {
    void refresh();
  });
</script>

<SectionHeader title="Schedules" sub="Time-of-day triggers that switch the active profile." />

<SettingRow
  label="Pause all schedules"
  sub="While paused, schedules don’t fire. Manual switching still works."
>
  <Toggle value={paused} onChange={togglePaused} />
</SettingRow>

<div class="rules-hdr">Rules</div>

{#if schedules.length === 0}
  <div class="empty">No schedules. Add one below.</div>
{:else}
  {#each schedules as r, i (i)}
    <ScheduleRow
      rule={r}
      {profileNames}
      onToggle={(v) => void toggleRule(i, v)}
      onEdit={() => editRule(i)}
      onDelete={() => void deleteRule(i)}
    />
  {/each}
{/if}

<button class="btn add-btn" onclick={newRule} disabled={loading}>
  <Icon name="plus" size={12} />
  Add schedule
</button>

{#if editing}
  <ScheduleEditor
    rule={editing.rule}
    {profileNames}
    isNew={editing.index === null}
    conflict={editConflict}
    onCancel={() => (editing = null)}
    onChange={(r) => {
      if (editing) editing.rule = r;
    }}
    onSave={(r) => void saveEdit(r)}
  />
{/if}

<style>
  .rules-hdr {
    margin-top: 16px;
    margin-bottom: 8px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--text-3);
    text-transform: uppercase;
  }
  .empty {
    font-size: 12px;
    color: var(--text-3);
    padding: 12px 0;
  }
  .add-btn {
    margin-top: 16px;
  }
</style>
