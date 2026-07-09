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
    // Twenty-four entries at a sixteenth of the budget each: 1.5x over, so the
    // least-recently-used are dropped until it fits. Sized so eviction is
    // driven by bytes and settles well above CACHE_MIN_ENTRIES — the floor must
    // not be what stops the loop here, or this would pass for the wrong reason.
    ctx.invokeImpl = () => Promise.resolve({ data: bigData(mod.CACHE_MAX_BYTES / 16), mime });
    for (let i = 0; i < 24; i += 1) await mod.loadSourceImage(`/img/${i}.png`);

    const held = Array.from({ length: 24 }, (_, i) => mod.peekSourceImage(`/img/${i}.png`)).filter(
      (e) => e !== null,
    );
    const retained = held.reduce((sum, e) => sum + e.url.length, 0);
    expect(retained).toBeLessThanOrEqual(mod.CACHE_MAX_BYTES);
    expect(held.length).toBeGreaterThan(mod.CACHE_MIN_ENTRIES);
    expect(mod.peekSourceImage('/img/0.png')).toBeNull();
    expect(mod.peekSourceImage('/img/23.png')).not.toBeNull();
  });

  it('keeps_a_floor_of_entries_when_each_alone_exceeds_the_budget', async () => {
    const mod = await import('./source-image');
    // Pathological sources (huge, incompressible) must not leave the cache
    // empty and re-fetching on every render: each pair alone busts the budget,
    // but the floor holds CACHE_MIN_ENTRIES of them.
    ctx.invokeImpl = () => Promise.resolve({ data: bigData(mod.CACHE_MAX_BYTES / 4), mime });
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
    //
    // Weights are realistic — a 1536px canvas render is ~683 KiB of base64, a
    // 512px swatch ~162 KiB — so the eight layers plus ten swatches sit inside
    // both bounds and nothing is evicted. This is a headroom guarantee.
    ctx.invokeImpl = () => Promise.resolve({ data: bigData(683 * 1024), mime });
    for (let i = 0; i < 8; i += 1) {
      await mod.loadSourceImage(`/img/layer-${i}.png`, mod.CANVAS_MAX_EDGE);
    }
    ctx.invokeImpl = () => Promise.resolve({ data: bigData(162 * 1024), mime });
    for (let i = 0; i < 10; i += 1) {
      await mod.loadSourceImage(`/img/profile-${i}.png`, mod.PREVIEW_MAX_EDGE);
    }
    for (let i = 0; i < 8; i += 1) {
      expect(mod.peekSourceImage(`/img/layer-${i}.png`, mod.CANVAS_MAX_EDGE)).not.toBeNull();
    }
  });

  it('recently_read_entries_outlive_older_unread_ones_under_byte_pressure', async () => {
    const mod = await import('./source-image');
    // `touch` on read makes eviction follow use, not insertion. Without it,
    // entry 0 — the one just read — would be the first evicted.
    ctx.invokeImpl = () => Promise.resolve({ data: bigData(mod.CACHE_MAX_BYTES / 8), mime });
    for (let i = 0; i < 8; i += 1) await mod.loadSourceImage(`/img/${i}.png`);

    expect(mod.peekSourceImage('/img/0.png')).not.toBeNull(); // read → promoted
    for (let i = 8; i < 12; i += 1) await mod.loadSourceImage(`/img/${i}.png`);

    expect(mod.peekSourceImage('/img/0.png')).not.toBeNull();
    expect(mod.peekSourceImage('/img/1.png')).toBeNull(); // never read → evicted
  });

  it('a_rejected_load_does_not_evict_a_newer_entry_at_the_same_key', async () => {
    const mod = await import('./source-image');
    // `prune()` can drop an in-flight entry (bytes are 0 until it settles).
    // A re-request then installs a fresh entry under the same key; the original
    // rejection must not delete the replacement.
    let rejectFirst: (e: Error) => void = () => undefined;
    ctx.invokeImpl = () => new Promise((_, rej) => (rejectFirst = rej));
    const failing = mod.loadSourceImage('/img/x.png');
    failing.catch(() => undefined); // settle later; don't trip an unhandled rejection

    // Push past the byte budget so prune() evicts the oldest entry — which is
    // the still-pending one above, counted at zero bytes.
    ctx.invokeImpl = () => Promise.resolve({ data: bigData(mod.CACHE_MAX_BYTES / 8), mime });
    for (let i = 0; i < 12; i += 1) await mod.loadSourceImage(`/img/big-${i}.png`);

    // Re-request the evicted key: a fresh entry lands under it.
    ctx.invokeImpl = () => Promise.resolve({ data: 'AAAA', mime });
    const replacement = await mod.loadSourceImage('/img/x.png');
    expect(mod.peekSourceImage('/img/x.png')).toEqual(replacement);

    // The original promise now rejects. It must not take the replacement out.
    rejectFirst(new Error('boom'));
    await expect(failing).rejects.toThrow('boom');
    expect(mod.peekSourceImage('/img/x.png')).toEqual(replacement);
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
