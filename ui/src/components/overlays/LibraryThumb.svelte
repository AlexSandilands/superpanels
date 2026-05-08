<script lang="ts">
  import { onMount } from 'svelte';
  import { getLibraryThumbUrl } from '$lib/library/thumb_cache';

  type Props = { path: string; alt: string };
  let { path, alt }: Props = $props();

  let url = $state<string | null>(null);
  let failed = $state(false);

  onMount(() => {
    let cancelled = false;
    failed = false;
    url = null;
    getLibraryThumbUrl(path)
      .then((u) => {
        if (!cancelled) url = u;
      })
      .catch(() => {
        if (!cancelled) failed = true;
      });
    return () => {
      cancelled = true;
    };
  });
</script>

{#if url}
  <img src={url} {alt} class="h-full w-full object-cover" decoding="async" />
{:else if failed}
  <div
    class="flex h-full w-full items-center justify-center"
    style:font-size="10px"
    style:color="var(--danger)"
  >
    failed
  </div>
{:else}
  <div
    class="flex h-full w-full items-center justify-center"
    style:font-size="10px"
    style:color="var(--text-3)"
  >
    …
  </div>
{/if}
