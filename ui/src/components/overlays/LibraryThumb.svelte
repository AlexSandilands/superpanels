<script lang="ts">
  import { onMount } from 'svelte';
  import { getLibraryThumbUrl } from '$lib/library/thumb-cache';

  type Props = { path: string; alt: string };
  let { path, alt }: Props = $props();

  let url = $state<string | null>(null);
  let failed = $state(false);
  let host = $state<HTMLElement>();

  // Defer thumbnail generation until the cell scrolls into view: a grid with
  // hundreds of members (the slideshow jump popover, the library) would
  // otherwise fire every decode at once on open.
  onMount(() => {
    let cancelled = false;
    let started = false;
    const load = () => {
      if (started) return;
      started = true;
      getLibraryThumbUrl(path)
        .then((u) => {
          if (!cancelled) url = u;
        })
        .catch(() => {
          if (!cancelled) failed = true;
        });
    };

    if (!host) {
      load();
      return () => {
        cancelled = true;
      };
    }
    const io = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          load();
          io.disconnect();
        }
      },
      { rootMargin: '150px' },
    );
    io.observe(host);
    return () => {
      cancelled = true;
      io.disconnect();
    };
  });
</script>

<div bind:this={host} class="h-full w-full">
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
</div>
