import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Hoisted mock state — `vi.mock` factories run before module init, so we
// keep the test-controllable bits in a `vi.hoisted` block.
const ctx = vi.hoisted(() => {
  return {
    invocations: [] as string[],
    edges: [] as (number | undefined)[],
    invokeImpl: (): Promise<{ data: string; mime: string }> =>
      Promise.resolve({ data: 'AAAA', mime: 'image/png' }),
  };
});

vi.mock('$lib/api', () => ({
  api: {
    sourceThumbnail: (path: string, maxEdge?: number) => {
      ctx.invocations.push(path);
      ctx.edges.push(maxEdge);
      return ctx.invokeImpl();
    },
  },
}));

const mime = 'image/png';

// The cache measures an entry by its `data:` URL length, so payload size is
// the only thing these fixtures need to control.
function bigData(bytes: number): string {
  return 'A'.repeat(Math.ceil(bytes));
}

// jsdom's Image doesn't actually decode `data:` URLs — stub it so `onload`
// fires synchronously after `src` is set with predictable natural dims.
class FakeImage {
  decoding = 'sync';
  onload: () => void = () => undefined;
  onerror: () => void = () => undefined;
  naturalWidth = 100;
  naturalHeight = 50;
  set src(_v: string) {
    void _v;
    queueMicrotask(() => this.onload());
  }
}

beforeEach(() => {
  ctx.invocations.length = 0;
  ctx.edges.length = 0;
  ctx.invokeImpl = () => Promise.resolve({ data: 'AAAA', mime: 'image/png' });
  vi.stubGlobal('Image', FakeImage);
  // Reset the module's cache by re-importing fresh.
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('loadSourceImage', () => {
  it('dedupes_concurrent_loads_for_same_path', async () => {
    const mod = await import('./source-image');
    // Two concurrent calls should issue exactly one IPC and resolve to the
    // same value. (Async functions wrap their return in a new Promise on
    // each call, so identity-equality on the returned Promise wouldn't
    // measure dedupe — the IPC-call count does.)
    const [a, b] = await Promise.all([
      mod.loadSourceImage('/img/a.png'),
      mod.loadSourceImage('/img/a.png'),
    ]);
    expect(ctx.invocations).toEqual(['/img/a.png']);
    expect(a).toEqual(b);
  });

  it('returns_decoded_image_with_natural_dims', async () => {
    const mod = await import('./source-image');
    const img = await mod.loadSourceImage('/img/a.png');
    expect(img.naturalW).toBe(100);
    expect(img.naturalH).toBe(50);
    expect(img.url.startsWith('data:image/png;base64,')).toBe(true);
  });

  it('peekSourceImage_returns_loaded_after_resolution', async () => {
    const mod = await import('./source-image');
    expect(mod.peekSourceImage('/img/a.png')).toBeNull();
    await mod.loadSourceImage('/img/a.png');
    const peeked = mod.peekSourceImage('/img/a.png');
    expect(peeked?.naturalW).toBe(100);
  });

  it('evicts_oldest_when_retained_bytes_exceed_budget', async () => {
    const mod = await import('./source-image');
    // Twelve entries at an eighth of the budget each: 1.5x over, so the
    // oldest are dropped until it fits. Sized so eviction is driven by bytes
    // and settles well above CACHE_MIN_ENTRIES — the floor must not be what
    // stops the loop here, or this would pass for the wrong reason.
    ctx.invokeImpl = () => Promise.resolve({ data: bigData(mod.CACHE_MAX_BYTES / 8), mime });
    for (let i = 0; i < 12; i += 1) await mod.loadSourceImage(`/img/${i}.png`);

    const held = Array.from({ length: 12 }, (_, i) => mod.peekSourceImage(`/img/${i}.png`)).filter(
      (e) => e !== null,
    );
    const retained = held.reduce((sum, e) => sum + e.url.length, 0);
    expect(retained).toBeLessThanOrEqual(mod.CACHE_MAX_BYTES);
    expect(held.length).toBeGreaterThan(mod.CACHE_MIN_ENTRIES);
    expect(mod.peekSourceImage('/img/0.png')).toBeNull();
    expect(mod.peekSourceImage('/img/11.png')).not.toBeNull();
  });

  it('keeps_a_floor_of_entries_when_each_alone_exceeds_the_budget', async () => {
    const mod = await import('./source-image');
    // A pathological source (huge, incompressible) must not leave the cache
    // empty and re-fetching on every render.
    ctx.invokeImpl = () => Promise.resolve({ data: bigData(mod.CACHE_MAX_BYTES), mime });
    for (let i = 0; i < mod.CACHE_MIN_ENTRIES + 2; i += 1) {
      await mod.loadSourceImage(`/img/${i}.png`);
    }
    const held = Array.from({ length: mod.CACHE_MIN_ENTRIES + 2 }, (_, i) =>
      mod.peekSourceImage(`/img/${i}.png`),
    ).filter((e) => e !== null);
    expect(held.length).toBe(mod.CACHE_MIN_ENTRIES);
  });

  it('canvas_entries_survive_a_burst_of_preview_sized_inserts', async () => {
    const mod = await import('./source-image');
    // Regression: with a fixed entry count, opening the profile manager (a
    // burst of cheap 512px inserts) evicted the canvas's expensive 1536px
    // renders, silently re-paying a full IPC round-trip per layer.
    for (let i = 0; i < 8; i += 1) {
      await mod.loadSourceImage(`/img/layer-${i}.png`, mod.CANVAS_MAX_EDGE);
    }
    for (let i = 0; i < 10; i += 1) {
      await mod.loadSourceImage(`/img/profile-${i}.png`, mod.PREVIEW_MAX_EDGE);
    }
    for (let i = 0; i < 8; i += 1) {
      expect(mod.peekSourceImage(`/img/layer-${i}.png`, mod.CANVAS_MAX_EDGE)).not.toBeNull();
    }
  });

  it('requests_the_default_edge_when_caller_omits_one', async () => {
    const mod = await import('./source-image');
    await mod.loadSourceImage('/img/a.png');
    expect(ctx.edges).toEqual([mod.PREVIEW_MAX_EDGE]);
  });

  it('same_path_at_two_edges_is_cached_separately', async () => {
    const mod = await import('./source-image');
    // The canvas asks for a big render of the same file the profile modal
    // shows as a swatch; neither may be served the other's copy.
    await mod.loadSourceImage('/img/a.png', mod.PREVIEW_MAX_EDGE);
    await mod.loadSourceImage('/img/a.png', mod.CANVAS_MAX_EDGE);
    expect(ctx.edges).toEqual([mod.PREVIEW_MAX_EDGE, mod.CANVAS_MAX_EDGE]);
    expect(mod.peekSourceImage('/img/a.png', mod.CANVAS_MAX_EDGE)).not.toBeNull();
    expect(mod.peekSourceImage('/img/a.png', 999)).toBeNull();
  });

  it('on_error_evicts_failed_entry_so_retry_refetches', async () => {
    const mod = await import('./source-image');
    // First call rejects.
    ctx.invokeImpl = () => Promise.reject(new Error('boom'));
    await expect(mod.loadSourceImage('/img/x.png')).rejects.toThrow('boom');
    // Second call should hit the IPC again (entry was evicted on rejection).
    ctx.invokeImpl = () => Promise.resolve({ data: 'BBBB', mime: 'image/png' });
    await mod.loadSourceImage('/img/x.png');
    expect(ctx.invocations).toEqual(['/img/x.png', '/img/x.png']);
  });
});
