<script lang="ts">
  import { profileStore, defaultSlideshowConfig } from '$lib/stores/profile.svelte';
  import { isSpanBody, type SlideshowSort, type SlideshowStart } from '$lib/types/profile';
  import { open } from '@tauri-apps/plugin-dialog';

  type Props = {
    source:
      | { type: 'single'; path: string }
      | {
          type: 'slideshow';
          images:
            | { type: 'folder'; path: string; recursive: boolean }
            | { type: 'playlist'; paths: string[] };
          config: ReturnType<typeof defaultSlideshowConfig>;
        };
  };
  let { source }: Props = $props();

  const sortModes: SlideshowSort[] = [
    'shuffle',
    'alphabetical',
    'date_asc',
    'date_desc',
    'last_shown_asc',
  ];
  const startModes: SlideshowStart[] = ['resume', 'new_random', 'first'];

  function isSortMode(v: string): v is SlideshowSort {
    return (sortModes as string[]).includes(v);
  }
  function isStartMode(v: string): v is SlideshowStart {
    return (startModes as string[]).includes(v);
  }

  function setSpanSource(kind: 'single' | 'slideshow') {
    profileStore.patchDraft((d) => {
      if (!isSpanBody(d.body)) return;
      if (kind === 'single' && d.body.source.type !== 'single') {
        d.body.source = { type: 'single', path: '' };
      } else if (kind === 'slideshow' && d.body.source.type !== 'slideshow') {
        d.body.source = {
          type: 'slideshow',
          images: { type: 'folder', path: '', recursive: true },
          config: defaultSlideshowConfig(),
        };
      }
    });
  }

  async function browseImage() {
    const file = await pickPath('image', source.type === 'single' ? source.path : undefined);
    if (!file) return;
    profileStore.patchDraft((d) => {
      if (isSpanBody(d.body) && d.body.source.type === 'single') {
        d.body.source.path = file;
      }
    });
  }

  async function browseFolder() {
    const folder = await pickPath(
      'folder',
      source.type === 'slideshow' && source.images.type === 'folder'
        ? source.images.path
        : undefined,
    );
    if (!folder) return;
    profileStore.patchDraft((d) => {
      if (
        isSpanBody(d.body) &&
        d.body.source.type === 'slideshow' &&
        d.body.source.images.type === 'folder'
      ) {
        d.body.source.images.path = folder;
      }
    });
  }

  async function pickPath(kind: 'image' | 'folder', currentPath?: string): Promise<string | null> {
    // exactOptionalPropertyTypes: omit `defaultPath` entirely when undefined —
    // passing the literal `undefined` is a type error.
    const defaultPath = currentPath !== undefined ? { defaultPath: currentPath } : {};
    const picked =
      kind === 'image'
        ? await open({
            multiple: false,
            directory: false,
            ...defaultPath,
            filters: [
              {
                name: 'Images',
                extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif', 'tif', 'tiff'],
              },
            ],
          })
        : await open({ multiple: false, directory: true, ...defaultPath });
    return typeof picked === 'string' ? picked : null;
  }

  function patchSlideshow(
    pick: (
      cfg: ReturnType<typeof defaultSlideshowConfig>,
      images:
        | { type: 'folder'; path: string; recursive: boolean }
        | { type: 'playlist'; paths: string[] },
    ) => void,
  ): void {
    profileStore.patchDraft((d) => {
      if (isSpanBody(d.body) && d.body.source.type === 'slideshow') {
        pick(d.body.source.config, d.body.source.images);
      }
    });
  }
</script>

<fieldset class="flex flex-col gap-2 rounded border border-slate-800 p-2">
  <legend class="px-1 text-[10px] uppercase tracking-wide text-slate-400">Source</legend>
  <div class="flex gap-1 text-xs">
    <button
      type="button"
      class="flex-1 rounded border px-2 py-1"
      class:border-accent={source.type === 'single'}
      class:border-slate-700={source.type !== 'single'}
      onclick={() => setSpanSource('single')}
    >
      Single image
    </button>
    <button
      type="button"
      class="flex-1 rounded border px-2 py-1"
      class:border-accent={source.type === 'slideshow'}
      class:border-slate-700={source.type !== 'slideshow'}
      onclick={() => setSpanSource('slideshow')}
    >
      Slideshow
    </button>
  </div>

  {#if source.type === 'single'}
    <div class="flex items-center gap-1">
      <input
        class="flex-1 rounded border border-slate-700 bg-slate-900/40 px-2 py-1 font-mono text-xs text-slate-100"
        placeholder="/path/to/image.jpg"
        value={source.path}
        oninput={(e) =>
          profileStore.patchDraft((d) => {
            if (isSpanBody(d.body) && d.body.source.type === 'single') {
              d.body.source.path = (e.currentTarget as HTMLInputElement).value;
            }
          })}
      />
      <button
        type="button"
        class="rounded border border-slate-700 px-2 py-1 text-xs hover:bg-slate-800"
        onclick={browseImage}
      >
        Browse…
      </button>
    </div>
  {:else if source.type === 'slideshow' && source.images.type === 'folder'}
    {@const folder = source.images}
    {@const cfg = source.config}
    <div class="flex items-center gap-1">
      <input
        class="flex-1 rounded border border-slate-700 bg-slate-900/40 px-2 py-1 font-mono text-xs text-slate-100"
        placeholder="/path/to/folder"
        value={folder.path}
        oninput={(e) =>
          patchSlideshow((_, images) => {
            if (images.type === 'folder') {
              images.path = (e.currentTarget as HTMLInputElement).value;
            }
          })}
      />
      <button
        type="button"
        class="rounded border border-slate-700 px-2 py-1 text-xs hover:bg-slate-800"
        onclick={browseFolder}
      >
        Browse…
      </button>
    </div>
    <label class="flex items-center gap-2 text-xs text-slate-300">
      <input
        type="checkbox"
        checked={folder.recursive}
        onchange={(e) =>
          patchSlideshow((_, images) => {
            if (images.type === 'folder') {
              images.recursive = (e.currentTarget as HTMLInputElement).checked;
            }
          })}
      />
      Recurse into subfolders
    </label>
    <div class="grid grid-cols-2 gap-2 text-xs">
      <label class="flex flex-col gap-1 text-slate-300">
        Interval (s)
        <input
          type="number"
          min="5"
          class="rounded border border-slate-700 bg-slate-900/40 px-2 py-1 text-slate-100"
          value={cfg.interval_secs}
          onchange={(e) =>
            patchSlideshow((c) => {
              const n = Number((e.currentTarget as HTMLInputElement).value);
              if (Number.isFinite(n) && n >= 5) c.interval_secs = Math.floor(n);
            })}
        />
      </label>
      <label class="flex flex-col gap-1 text-slate-300">
        Sort
        <select
          class="rounded border border-slate-700 bg-slate-900/40 px-2 py-1 text-slate-100"
          value={cfg.sort}
          onchange={(e) =>
            patchSlideshow((c) => {
              const v = (e.currentTarget as HTMLSelectElement).value;
              if (isSortMode(v)) c.sort = v;
            })}
        >
          {#each sortModes as s (s)}
            <option value={s}>{s}</option>
          {/each}
        </select>
      </label>
      <label class="flex flex-col gap-1 text-slate-300">
        On start
        <select
          class="rounded border border-slate-700 bg-slate-900/40 px-2 py-1 text-slate-100"
          value={cfg.on_start}
          onchange={(e) =>
            patchSlideshow((c) => {
              const v = (e.currentTarget as HTMLSelectElement).value;
              if (isStartMode(v)) c.on_start = v;
            })}
        >
          {#each startModes as s (s)}
            <option value={s}>{s}</option>
          {/each}
        </select>
      </label>
      <label class="flex flex-col gap-1 text-slate-300">
        History size
        <input
          type="number"
          min="0"
          class="rounded border border-slate-700 bg-slate-900/40 px-2 py-1 text-slate-100"
          value={cfg.recent_history_size}
          onchange={(e) =>
            patchSlideshow((c) => {
              const n = Number((e.currentTarget as HTMLInputElement).value);
              if (Number.isFinite(n) && n >= 0) c.recent_history_size = Math.floor(n);
            })}
        />
      </label>
    </div>
  {/if}
</fieldset>
