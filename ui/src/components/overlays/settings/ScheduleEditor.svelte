<script lang="ts">
  import type { Schedule } from '$lib/types/Schedule';
  import type { Trigger } from '$lib/types/Trigger';
  import Select from '../../widgets/Select.svelte';

  type TriggerKind = 'daily' | 'sun' | 'cron';
  type Props = {
    rule: Schedule;
    profileNames: string[];
    isNew: boolean;
    conflict: { display_name?: string | null } | null;
    onCancel: () => void;
    onSave: (rule: Schedule) => void;
    onChange: (rule: Schedule) => void;
  };
  let { rule, profileNames, isNew, conflict, onCancel, onSave, onChange }: Props = $props();

  function kindOf(t: Trigger): TriggerKind {
    if (t.type === 'daily') return 'daily';
    if (t.type === 'sunset' || t.type === 'sunrise') return 'sun';
    return 'cron';
  }

  function setKind(k: TriggerKind) {
    if (k === kindOf(rule.trigger)) return;
    let next: Trigger;
    if (k === 'daily') next = { type: 'daily', hour: 18, minute: 0 };
    else if (k === 'sun') next = { type: 'sunset', offset_minutes: 0 };
    else next = { type: 'cron', expr: '0 0 * * * *' };
    onChange({ ...rule, trigger: next });
  }

  function setEvent(v: string) {
    const event = v === 'sunrise' ? 'sunrise' : 'sunset';
    const offset =
      rule.trigger.type === 'sunset' || rule.trigger.type === 'sunrise'
        ? rule.trigger.offset_minutes
        : 0;
    onChange({ ...rule, trigger: { type: event, offset_minutes: offset } });
  }

  function setHour(e: Event) {
    if (rule.trigger.type !== 'daily') return;
    const v = parseInt((e.target as HTMLInputElement).value, 10);
    const hour = Number.isFinite(v) ? Math.max(0, Math.min(23, v)) : 0;
    onChange({ ...rule, trigger: { ...rule.trigger, hour } });
  }
  function setMinute(e: Event) {
    if (rule.trigger.type !== 'daily') return;
    const v = parseInt((e.target as HTMLInputElement).value, 10);
    const minute = Number.isFinite(v) ? Math.max(0, Math.min(59, v)) : 0;
    onChange({ ...rule, trigger: { ...rule.trigger, minute } });
  }
  function setOffset(e: Event) {
    if (rule.trigger.type !== 'sunset' && rule.trigger.type !== 'sunrise') return;
    const v = parseInt((e.target as HTMLInputElement).value, 10);
    onChange({
      ...rule,
      trigger: { ...rule.trigger, offset_minutes: Number.isFinite(v) ? v : 0 },
    });
  }
  function setCron(e: Event) {
    if (rule.trigger.type !== 'cron') return;
    onChange({
      ...rule,
      trigger: { ...rule.trigger, expr: (e.target as HTMLInputElement).value },
    });
  }
  function setProfile(v: string) {
    onChange({ ...rule, profile: v });
  }
  function setName(e: Event) {
    onChange({ ...rule, display_name: (e.target as HTMLInputElement).value || null });
  }

  let kind = $derived(kindOf(rule.trigger));
  let profileOptions = $derived(profileNames.map((n) => ({ value: n, label: n })));
  let eventValue = $derived(rule.trigger.type === 'sunrise' ? 'sunrise' : 'sunset');
  let offsetValue = $derived(
    rule.trigger.type === 'sunset' || rule.trigger.type === 'sunrise'
      ? rule.trigger.offset_minutes
      : 0,
  );
</script>

<div class="editor panel">
  <div class="hdr">{isNew ? 'New schedule' : 'Edit schedule'}</div>

  <div class="grid">
    <span class="lbl">Trigger</span>
    <div class="seg" role="tablist">
      {#each [['daily', 'Daily'], ['sun', 'Sunset/Sunrise'], ['cron', 'Cron']] as [k, l] (k)}
        <button
          type="button"
          class="seg-btn"
          class:on={kind === k}
          role="tab"
          aria-selected={kind === k}
          onclick={() => setKind(k as TriggerKind)}
        >
          {l}
        </button>
      {/each}
    </div>

    {#if rule.trigger.type === 'daily'}
      <span class="lbl">Time</span>
      <div class="time">
        <input
          type="number"
          min="0"
          max="23"
          class="field mono"
          style:width="56px"
          value={rule.trigger.hour}
          oninput={setHour}
        />
        <span class="colon">:</span>
        <input
          type="number"
          min="0"
          max="59"
          class="field mono"
          style:width="56px"
          value={rule.trigger.minute}
          oninput={setMinute}
        />
      </div>
    {:else if rule.trigger.type === 'sunset' || rule.trigger.type === 'sunrise'}
      <span class="lbl">Event</span>
      <div class="event">
        <Select
          value={eventValue}
          options={[
            { value: 'sunset', label: 'Sunset' },
            { value: 'sunrise', label: 'Sunrise' },
          ]}
          onChange={setEvent}
          minWidth={120}
        />
        <input
          type="number"
          class="field mono"
          style:width="80px"
          value={offsetValue}
          oninput={setOffset}
        />
        <span class="hint">min offset</span>
      </div>
    {:else}
      <span class="lbl">Expression</span>
      <input
        type="text"
        class="field mono"
        style:width="100%"
        value={rule.trigger.expr}
        oninput={setCron}
      />
    {/if}

    <span class="lbl">Target</span>
    {#if profileOptions.length > 0}
      <Select value={rule.profile} options={profileOptions} onChange={setProfile} minWidth={200} />
    {:else}
      <span class="hint">No profiles available</span>
    {/if}

    <span class="lbl">Name</span>
    <input
      type="text"
      class="field ui"
      placeholder="optional"
      value={rule.display_name ?? ''}
      oninput={setName}
    />
  </div>

  {#if conflict}
    <div class="warn">
      <span class="warn-icon" aria-hidden="true">⚠</span>
      <span>
        Conflicts with another rule that fires at the same minute:
        <strong>{conflict.display_name || 'untitled rule'}</strong>
      </span>
    </div>
  {/if}

  <div class="actions">
    <button class="btn" onclick={onCancel}>Cancel</button>
    <button class="btn primary" disabled={!!conflict} onclick={() => onSave(rule)}>Save</button>
  </div>
</div>

<style>
  .editor {
    margin-top: 14px;
    padding: 14px;
    border-radius: 8px;
  }
  .hdr {
    font-size: 13px;
    font-weight: 600;
    margin-bottom: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 10px 14px;
    align-items: center;
    margin-bottom: 12px;
  }
  .lbl {
    font-size: 11px;
    color: var(--text-3);
  }
  .seg {
    display: inline-flex;
    border-radius: 6px;
    overflow: hidden;
    border: 1px solid var(--line);
    align-self: start;
  }
  .seg-btn {
    appearance: none;
    border: none;
    height: 26px;
    padding: 0 12px;
    font-size: 11px;
    font-family: inherit;
    background: transparent;
    color: var(--text-2);
    transition:
      background 80ms,
      color 80ms;
  }
  .seg-btn:hover:not(.on) {
    background: var(--panel-2);
  }
  .seg-btn.on {
    background: var(--accent);
    color: oklch(0.16 0.01 250);
  }
  .time,
  .event {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .colon {
    color: var(--text-3);
  }
  .hint {
    font-size: 11px;
    color: var(--text-3);
  }
  .warn {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 8px 10px;
    border-radius: 6px;
    font-size: 11px;
    background: color-mix(in oklab, var(--danger) 16%, var(--panel));
    border: 1px solid color-mix(in oklab, var(--danger) 40%, var(--line));
    color: var(--text);
    margin-bottom: 12px;
  }
  .warn-icon {
    color: var(--danger);
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
</style>
