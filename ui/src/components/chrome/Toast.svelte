<script lang="ts">
  import { toast, type Toast } from '$lib/stores/toast.svelte';

  $effect(() => {
    const live: Toast[] = toast.items.filter((t) => t.expiresAt !== undefined);
    if (live.length === 0) return;
    const now = Date.now();
    const next = live.reduce<Toast | undefined>((soonest, t) => {
      if (soonest === undefined || (t.expiresAt ?? 0) < (soonest.expiresAt ?? 0)) return t;
      return soonest;
    }, undefined);
    if (next === undefined || next.expiresAt === undefined) return;
    const delay = Math.max(0, next.expiresAt - now);
    const handle = window.setTimeout(() => toast.dismiss(next.id), delay);
    return () => window.clearTimeout(handle);
  });

  function severityClass(sev: 'success' | 'error' | 'info'): 'ok' | 'live' | 'danger' {
    if (sev === 'success') return 'ok';
    if (sev === 'error') return 'danger';
    return 'live';
  }
</script>

<div class="toast-stack">
  {#each toast.items as t (t.id)}
    <div class="toast">
      <span class="dot {severityClass(t.severity)}" style:margin-top="4px"></span>
      <div style:flex="1" style:min-width="0">
        <div style:font-size="12px" style:font-weight="600">{t.title}</div>
        {#if t.detail}
          <div
            class="mono"
            style:font-size="11px"
            style:color="var(--text-3)"
            style:margin-top="2px"
          >
            {t.detail}
          </div>
        {/if}
      </div>
      <button
        class="btn ghost icon sm"
        aria-label="Dismiss"
        style:color="var(--text-3)"
        onclick={() => toast.dismiss(t.id)}
      >
        ×
      </button>
    </div>
  {/each}
</div>
