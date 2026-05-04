<script lang="ts">
  import { profileStore } from '$lib/stores/profile.svelte';
  import type { Schedule } from '$lib/types/profile';

  type Props = { schedule: Schedule | undefined; profileName: string };
  let { schedule, profileName }: Props = $props();

  const kinds: Array<'none' | 'daily' | 'sunset' | 'cron'> = ['none', 'daily', 'sunset', 'cron'];

  function setKind(kind: 'none' | 'daily' | 'sunset' | 'cron') {
    profileStore.patchDraft((d) => {
      if (kind === 'none') {
        delete d.schedule;
        return;
      }
      if (kind === 'daily') {
        d.schedule = { type: 'daily', hour: 8, minute: 0, profile: profileName };
      } else if (kind === 'sunset') {
        d.schedule = { type: 'sunset', offset_minutes: 0, profile: profileName };
      } else {
        d.schedule = { type: 'cron', expr: '0 8 * * *' };
      }
    });
  }

  const cur = $derived(schedule?.type ?? 'none');
</script>

<fieldset class="flex flex-col gap-2 rounded border border-slate-800 p-2">
  <legend class="px-1 text-[10px] uppercase tracking-wide text-slate-400">Schedule</legend>
  <div class="flex gap-1 text-xs">
    {#each kinds as kind (kind)}
      <button
        type="button"
        class="flex-1 rounded border px-2 py-1 capitalize"
        class:border-accent={cur === kind}
        class:border-slate-700={cur !== kind}
        onclick={() => setKind(kind)}
      >
        {kind}
      </button>
    {/each}
  </div>
  {#if schedule?.type === 'daily'}
    {@const sch = schedule}
    <div class="flex items-center gap-2 text-xs text-slate-300">
      <input
        type="number"
        min="0"
        max="23"
        class="w-16 rounded border border-slate-700 bg-slate-900/40 px-2 py-1 text-slate-100"
        value={sch.hour}
        onchange={(e) =>
          profileStore.patchDraft((d) => {
            if (d.schedule?.type === 'daily') {
              const n = Number((e.currentTarget as HTMLInputElement).value);
              if (Number.isInteger(n) && n >= 0 && n <= 23) d.schedule.hour = n;
            }
          })}
      />
      <span>:</span>
      <input
        type="number"
        min="0"
        max="59"
        class="w-16 rounded border border-slate-700 bg-slate-900/40 px-2 py-1 text-slate-100"
        value={sch.minute}
        onchange={(e) =>
          profileStore.patchDraft((d) => {
            if (d.schedule?.type === 'daily') {
              const n = Number((e.currentTarget as HTMLInputElement).value);
              if (Number.isInteger(n) && n >= 0 && n <= 59) d.schedule.minute = n;
            }
          })}
      />
      <span class="text-slate-500">daily</span>
    </div>
  {:else if schedule?.type === 'sunset'}
    {@const sch = schedule}
    <label class="flex items-center gap-2 text-xs text-slate-300">
      <span class="w-32 shrink-0">Offset (minutes)</span>
      <input
        type="number"
        class="w-24 rounded border border-slate-700 bg-slate-900/40 px-2 py-1 text-slate-100"
        value={sch.offset_minutes}
        onchange={(e) =>
          profileStore.patchDraft((d) => {
            if (d.schedule?.type === 'sunset') {
              const n = Number((e.currentTarget as HTMLInputElement).value);
              if (Number.isFinite(n)) d.schedule.offset_minutes = Math.floor(n);
            }
          })}
      />
    </label>
  {:else if schedule?.type === 'cron'}
    {@const sch = schedule}
    <label class="flex items-center gap-2 text-xs text-slate-300">
      <span class="w-20 shrink-0">Expression</span>
      <input
        class="flex-1 rounded border border-slate-700 bg-slate-900/40 px-2 py-1 font-mono text-xs text-slate-100"
        placeholder="0 8 * * *"
        value={sch.expr}
        oninput={(e) =>
          profileStore.patchDraft((d) => {
            if (d.schedule?.type === 'cron') {
              d.schedule.expr = (e.currentTarget as HTMLInputElement).value;
            }
          })}
      />
    </label>
  {/if}
</fieldset>
