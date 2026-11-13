import { describe, it, expect } from 'vitest';
// @ts-expect-error Worker is intentionally plain ESM for Cloudflare and Node.
import worker, { mapRequest } from './worker.mjs';
describe('map boundary', () => {
  it('rejects missing, non-finite and oversized coordinates before requesting providers', () => {
    for (const query of ['', 'lat=NaN&lon=13', 'lat=86&lon=13', 'lat=52&lon=Infinity', 'lat=52&lon=13&x=1.5', 'lat=52&lon=13&y=9999']) {
      expect(() => mapRequest(new URL(`https://game.test/api/osm?${query}`))).toThrow();
    }
  });
  it('uses only bounded generated queries and stable public cache names', () => {
    const m = mapRequest(new URL('https://game.test/api/osm?lat=52.4275793&lon=13.741161&x=0&y=0&data=evil'));
    expect(m.file).toBe('52.42758_13.74116_0_0.json');
    expect(m.query).toContain('way["building"]'); expect(m.query).not.toContain('evil');
  });
  it('serves callbacks as HTML but does not turn missing scripts into a successful HTML response', async () => {
    const urls: string[] = [];
    const env = { ASSETS: { fetch: async (r: Request) => { urls.push(new URL(r.url).pathname); return new Response('', { status: 404 }); } } };
    await worker.fetch(new Request('https://game.test/identity/return'), env, {});
    const missing = await worker.fetch(new Request('https://game.test/missing.js'), env, {});
    expect(urls).toEqual(['/index.html', '/missing.js']); expect(missing.status).toBe(404);
    expect(missing.headers.get('referrer-policy')).toBe('strict-origin-when-cross-origin');
  });
});
