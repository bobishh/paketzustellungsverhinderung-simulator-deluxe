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


describe('WASM cache revalidation', () => {
  it.each(['GET', 'HEAD'])('preserves a compressed asset 304 for %s instead of returning 404', async method => {
    const env = { ASSETS: { fetch: async (request: Request) => new URL(request.url).pathname.endsWith('.gz')
      ? new Response(null, { status: 304, headers: { etag: '"release"' } })
      : new Response(null, { status: 404 }) } };
    const response = await worker.fetch(new Request('https://game.test/game/paket_game_bg.wasm', {
      method, headers: { 'if-none-match': '"release"' },
    }), env, {});
    expect(response.status).toBe(304);
    expect(response.headers.get('etag')).toBe('"release"');
    expect(response.headers.get('content-type')).toBe('application/wasm');
  });
  it('does not cache missing runtime assets', async () => {
    const env = { ASSETS: { fetch: async () => new Response(null, { status: 404 }) } };
    const response = await worker.fetch(new Request('https://game.test/mesh/meta_mesh_bg.wasm'), env, {});
    expect(response.headers.get('cache-control')).toBe('no-store');
  });
});

describe('public mesh directory', () => {
  const invite = { protocol:'paket-web/1', origin:{lat:52.4895,lon:13.3908}, host:'a'.repeat(64), room:'b'.repeat(64), relay:'https://relay.example' };
  it('lists live meshes from bounded directory partitions', async () => {
    const env = { MESH_DIRECTORY:{getByName:()=>({list:async()=>[invite]})} };
    const response = await worker.fetch(new Request('https://game.test/api/meshes'),env,{});
    expect(response.status).toBe(200);
    expect((await response.json()).length).toBeGreaterThan(0);
    expect(response.headers.get('cache-control')).toBe('no-store');
  });
  it('rejects a stranger trying to publish or delete another room', async () => {
    for (const method of ['PUT','DELETE']) {
      const request = new Request('https://game.test/api/meshes',{method,headers:{origin:'https://game.test',authorization:`Bearer ${'a'.repeat(64)}`},body:JSON.stringify(invite)});
      const response = await worker.fetch(request,{},{});
      expect(response.status).toBe(403);
    }
  });
});


it('publishes only an owner-authorized, bounded public invitation', async () => {
  const token = 'd'.repeat(64);
  const room = [...new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(token)))].map(x=>x.toString(16).padStart(2,'0')).join('');
  const invite = {protocol:'paket-web/1',room,host:'a'.repeat(64),origin:{lat:52.4895,lon:13.3908},relay:'https://relay.example'};
  const published: unknown[] = [];
  const env = {MESH_DIRECTORY:{getByName:()=>({publish:async (i: unknown)=>{published.push(i);return true;}})}};
  const request = (body: string, origin = 'https://game.test') => new Request('https://game.test/api/meshes',{method:'PUT',headers:{origin,authorization:`Bearer ${token}`},body});
  expect((await worker.fetch(request(JSON.stringify(invite)),env,{})).status).toBe(200);
  expect(published).toEqual([invite]);
  expect((await worker.fetch(request(JSON.stringify({...invite,origin:{lat:Infinity,lon:0}})),env,{})).status).toBe(400);
  expect((await worker.fetch(request('x'.repeat(4097)),env,{})).status).toBe(413);
  expect((await worker.fetch(request(JSON.stringify(invite),'https://elsewhere.test'),env,{})).status).toBe(403);
  expect(published).toHaveLength(1);
});
