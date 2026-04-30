<script lang="ts">
  import { toast, type Toast } from '$lib/stores/toast.svelte';

  // Schedule expiry inside an $effect so the timer is tied to the component
  // lifecycle and torn down with it (style-frontend.md "Forbidden patterns").
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
</script>

<div class="pointer-events-none fixed bottom-4 right-4 z-50 flex w-96 flex-col gap-2">
  {#each toast.items as t (t.id)}
    <div
      class="pointer-events-auto rounded border bg-slate-900 px-3 py-2 shadow-lg"
      class:border-red-500={t.severity === 'error'}
      class:border-emerald-500={t.severity === 'success'}
      class:border-slate-600={t.severity === 'info'}
    >
      <div class="flex items-start justify-between gap-2">
        <div class="min-w-0 flex-1">
          <div class="text-sm font-medium text-slate-100">{t.title}</div>
          {#if t.detail}
            <div class="mt-1 break-words text-xs text-slate-400">{t.detail}</div>
          {/if}
        </div>
        <button
          type="button"
          class="rounded px-2 text-xs text-slate-400 hover:bg-slate-800"
          onclick={() => toast.dismiss(t.id)}
          aria-label="Dismiss"
        >
          ✕
        </button>
      </div>
    </div>
  {/each}
</div>
