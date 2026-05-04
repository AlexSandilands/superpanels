<script lang="ts">
  import { onMount } from 'svelte';
  import { profileStore } from '$lib/stores/profile.svelte';

  onMount(() => {
    void profileStore.refresh();
  });
</script>

<div class="flex h-full flex-col gap-2">
  <div class="flex items-center justify-between">
    <h2 class="text-sm font-semibold text-slate-300">Profiles</h2>
    <div class="flex gap-1">
      <button
        type="button"
        class="rounded border border-slate-700 px-2 py-0.5 text-xs hover:bg-slate-800"
        onclick={() => profileStore.newProfile()}
      >
        New
      </button>
      <button
        type="button"
        class="rounded border border-slate-700 px-2 py-0.5 text-xs hover:bg-slate-800"
        onclick={() => profileStore.refresh()}
        disabled={profileStore.loading}
      >
        {profileStore.loading ? 'Refreshing…' : 'Refresh'}
      </button>
    </div>
  </div>

  {#if profileStore.profiles.length === 0}
    <p class="text-xs text-slate-500">
      No profiles yet — click "New" or use
      <code class="rounded bg-slate-800 px-1">superpanels set --save-as</code>.
    </p>
  {:else}
    <ul class="flex flex-col gap-1 overflow-y-auto">
      {#each profileStore.profiles as profile (profile.name)}
        {@const isSelected = profileStore.selectedName === profile.name}
        {@const isActive = profileStore.activeName === profile.name}
        <li
          class="flex items-center justify-between rounded border px-2 py-1.5 transition-colors"
          class:border-accent={isSelected}
          class:bg-slate-800={isSelected}
          class:border-slate-800={!isSelected}
          class:bg-slate-900={!isSelected}
        >
          <button
            type="button"
            class="flex flex-1 items-center gap-2 truncate text-left"
            onclick={() => profileStore.select(profile.name)}
          >
            <span class="font-mono text-sm text-slate-200">{profile.name}</span>
            {#if isActive}
              <span
                class="rounded bg-accent/20 px-1.5 py-0.5 text-[10px] font-semibold text-accent"
              >
                ACTIVE
              </span>
            {/if}
          </button>
          <div class="flex gap-1">
            <button
              type="button"
              class="rounded bg-accent/80 px-2 py-0.5 text-xs text-slate-900 hover:bg-accent"
              onclick={() => profileStore.apply(profile.name)}
            >
              Apply
            </button>
            <button
              type="button"
              class="rounded border border-slate-700 px-2 py-0.5 text-xs text-slate-300 hover:bg-slate-800"
              onclick={() => profileStore.delete(profile.name)}
            >
              Delete
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>
