import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Hoisted mock state — `vi.mock` factories run before module init, so we
// keep the test-controllable bits in a `vi.hoisted` block.
const ctx = vi.hoisted(() => {
  return {
    invocations: [] as string[],
    invokeImpl: (): Promise<{ data: string; mime: string }> =>
      Promise.resolve({ data: 'AAAA', mime: 'image/png' }),
  };
});

vi.mock('$lib/api', () => ({
  api: {
    sourceThumbnail: (path: string) => {
      ctx.invocations.push(path);
      return ctx.invokeImpl();
    },
  },
}));

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
    const mod = await import('./sourceImage');
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
    const mod = await import('./sourceImage');
    const img = await mod.loadSourceImage('/img/a.png');
    expect(img.naturalW).toBe(100);
    expect(img.naturalH).toBe(50);
    expect(img.url.startsWith('data:image/png;base64,')).toBe(true);
  });

  it('peekSourceImage_returns_loaded_after_resolution', async () => {
    const mod = await import('./sourceImage');
    expect(mod.peekSourceImage('/img/a.png')).toBeNull();
    await mod.loadSourceImage('/img/a.png');
    const peeked = mod.peekSourceImage('/img/a.png');
    expect(peeked?.naturalW).toBe(100);
  });

  it('lru_evicts_oldest_when_over_capacity', async () => {
    const mod = await import('./sourceImage');
    // MAX_ENTRIES is 16; load 17 distinct paths and the first should evict.
    for (let i = 0; i < 17; i += 1) {
      await mod.loadSourceImage(`/img/${i}.png`);
    }
    expect(mod.peekSourceImage('/img/0.png')).toBeNull();
    expect(mod.peekSourceImage('/img/16.png')).not.toBeNull();
  });

  it('on_error_evicts_failed_entry_so_retry_refetches', async () => {
    const mod = await import('./sourceImage');
    // First call rejects.
    ctx.invokeImpl = () => Promise.reject(new Error('boom'));
    await expect(mod.loadSourceImage('/img/x.png')).rejects.toThrow('boom');
    // Second call should hit the IPC again (entry was evicted on rejection).
    ctx.invokeImpl = () => Promise.resolve({ data: 'BBBB', mime: 'image/png' });
    await mod.loadSourceImage('/img/x.png');
    expect(ctx.invocations).toEqual(['/img/x.png', '/img/x.png']);
  });
});
