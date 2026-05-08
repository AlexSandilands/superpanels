<script lang="ts">
  import { profileStore } from '$lib/stores/profile.svelte';
  import SectionHeader from './SectionHeader.svelte';

  type ScheduleRow = { profile: string; when: string };

  function describe(
    profile: string,
    schedule: NonNullable<(typeof profileStore.profiles)[number]['schedule']>,
  ): string {
    if (schedule.type === 'daily')
      return `Daily at ${String(schedule.hour).padStart(2, '0')}:${String(schedule.minute).padStart(2, '0')}`;
    if (schedule.type === 'sunset') {
      const off = schedule.offset_minutes;
      const sign = off >= 0 ? '+' : '−';
      return `Sunset ${sign}${Math.abs(off)} min → ${profile}`;
    }
    return `cron: ${schedule.expr}`;
  }

  const rows = $derived<ScheduleRow[]>(
    profileStore.profiles
      .filter((p) => p.schedule)
      .map((p) => ({ profile: p.name, when: p.schedule ? describe(p.name, p.schedule) : '' })),
  );
</script>

<SectionHeader title="Schedules" sub="Time-of-day triggers that switch the active profile." />

{#if rows.length === 0}
  <div style:padding="14px" style:font-size="12px" style:color="var(--text-3)">
    No schedules defined yet. Schedules are set per-profile in the config file (
    <code class="mono">[profiles.&lt;name&gt;.schedule]</code>).
  </div>
{:else}
  {#each rows as r (r.profile)}
    <div class="row">
      <div style:flex="1">
        <div style:font-size="13px" style:font-weight="500">{r.when}</div>
        <div class="mono" style:font-size="11px" style:color="var(--text-3)" style:margin-top="2px">
          → {r.profile}
        </div>
      </div>
    </div>
  {/each}
{/if}

<div style:margin-top="14px" style:font-size="11px" style:color="var(--text-3)">
  In-app schedule editing is not yet wired in this build.
</div>

<style>
  .row {
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    gap: 14px;
  }
</style>
