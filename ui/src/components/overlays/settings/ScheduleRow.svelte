<script lang="ts">
  import type { Schedule } from '$lib/types/Schedule';
  import type { Trigger } from '$lib/types/Trigger';
  import Toggle from './Toggle.svelte';

  type Props = {
    rule: Schedule;
    profileNames: string[];
    onToggle: (v: boolean) => void;
    onEdit: () => void;
    onDelete: () => void;
  };
  let { rule, profileNames, onToggle, onEdit, onDelete }: Props = $props();

  function summarise(t: Trigger): string {
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

  let summary = $derived(summarise(rule.trigger));
  let title = $derived(rule.display_name || summary);
  let targetExists = $derived(profileNames.includes(rule.profile));
</script>

<div class="row">
  <Toggle value={rule.enabled} onChange={onToggle} />
  <div class="body">
    <div class="title">{title}</div>
    <div class="meta mono">
      <span>{summary}</span>
      <span class="dot-sep">·</span>
      <span class="target" class:missing={!targetExists}>
        {targetExists ? rule.profile : `${rule.profile} (missing)`}
      </span>
    </div>
  </div>
  <button class="btn sm" onclick={onEdit}>Edit</button>
  <button class="btn sm icon danger" title="Delete" aria-label="Delete rule" onclick={onDelete}>
    ×
  </button>
</div>

<style>
  .row {
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 13px;
    font-weight: 500;
  }
  .meta {
    margin-top: 2px;
    font-size: 11px;
    color: var(--text-3);
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .dot-sep {
    opacity: 0.6;
  }
  .target {
    color: var(--text-2);
  }
  .target.missing {
    color: var(--danger);
  }
  .btn.icon {
    font-size: 14px;
    line-height: 1;
  }
</style>
