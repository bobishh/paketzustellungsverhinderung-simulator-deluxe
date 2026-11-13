const headers = { 'x-content-type-options': 'nosniff', 'referrer-policy': 'strict-origin-when-cross-origin', 'x-frame-options': 'DENY' };
export function mapRequest(url) {
  const p = url.searchParams;
  if (!p.has('lat') || !p.has('lon')) throw new Error('Missing coordinates');
  const lat = Number(p.get('lat')), lon = Number(p.get('lon'));
  const x = Number(p.get('x') ?? 0), y = Number(p.get('y') ?? 0), landscape = p.get('landscape') === '1';
  if (!Number.isFinite(lat) || Math.abs(lat) > 85 || !Number.isFinite(lon) || Math.abs(lon) > 180 || !Number.isInteger(x) || !Number.isInteger(y) || Math.abs(x) > 500 || Math.abs(y) > 500) throw new Error('Invalid coordinates');
  const project = (e, n) => [lat + n / 6378137 * 180 / Math.PI, lon + e / (6378137 * Math.cos(lat * Math.PI / 180)) * 180 / Math.PI];
  const sw = landscape ? project(-1650, -1650) : project(x * 180, y * 180);
  const ne = landscape ? project(1650, 1650) : project((x + 1) * 180, (y + 1) * 180);
  const bbox = [...sw, ...ne].map(n => n.toFixed(6)).join(',');
  const query = landscape ? `[out:json][timeout:16];(wr["natural"~"^(water|wood)$"](${bbox});wr["landuse"="forest"](${bbox});wr["waterway"="riverbank"](${bbox}););out geom;` :
    `[out:json][timeout:25];(way["building"](${bbox});way["highway"](${bbox}););out geom;`;
  return { query, file: `${lat.toFixed(5)}_${lon.toFixed(5)}_${landscape ? 'landscape' : `${x}_${y}`}.json` };
}
export default {
  async fetch(request, env, ctx) {
    if (!['GET', 'HEAD'].includes(request.method)) return new Response('Method not allowed', { status: 405, headers: { ...headers, allow: 'GET, HEAD' } });
    const url = new URL(request.url);
    if (url.pathname === '/api/osm') {
      let map;
      try { map = mapRequest(url); } catch { return new Response('Invalid map request', { status: 400, headers }); }
      const assetUrl = new URL(`/osm/${map.file}`, url);
      const bundled = await env.ASSETS.fetch(new Request(assetUrl));
      if (bundled.ok) return new Response(request.method === 'HEAD' ? null : bundled.body, { headers: { ...headers, 'content-type': 'application/json', 'cache-control': 'public,max-age=86400' } });
      const cacheKey = new Request(new URL(`/map-cache/${map.file}`, url));
      const cached = await caches.default.match(cacheKey);
      if (cached) return request.method === 'HEAD' ? new Response(null, cached) : cached;
      for (const endpoint of ['https://overpass.openstreetmap.fr/api/interpreter', 'https://overpass.kumi.systems/api/interpreter']) {
        try {
          const upstream = await fetch(endpoint, { method: 'POST', body: new URLSearchParams({ data: map.query }), headers: { 'user-agent': 'Paketzustellungsverhinderung/0.1 (OSM neighbourhood game)' }, signal: AbortSignal.timeout(28000) });
          if (!upstream.ok) continue;
          const body = await upstream.text();
          if (body.length > 16000000 || !Array.isArray(JSON.parse(body).elements)) continue;
          const response = new Response(body, { headers: { ...headers, 'content-type': 'application/json', 'cache-control': 'public,max-age=86400' } });
          ctx.waitUntil(caches.default.put(cacheKey, response.clone()));
          return request.method === 'HEAD' ? new Response(null, response) : response;
        } catch { /* Try the next map provider. */ }
      }
      return new Response('Map provider is busy. Please retry.', { status: 503, headers: { ...headers, 'retry-after': '15' } });
    }
    const route = ['/', '/identity/return'].includes(url.pathname) ? new URL('/index.html', url) : url;
    let response = await env.ASSETS.fetch(new Request(route, request));
    if (route.pathname.endsWith('.wasm') && response.status === 404) {
      const compressed = new URL(route); compressed.pathname += '.gz';
      const asset = await env.ASSETS.fetch(new Request(compressed, request));
      if (asset.ok) {
        const encoded = new Headers(asset.headers);
        encoded.set('content-type', 'application/wasm'); encoded.set('content-encoding', 'gzip');
        response = new Response(asset.body, { status: asset.status, headers: encoded });
      }
    }
    const merged = new Headers(response.headers);
    for (const [key, value] of Object.entries(headers)) merged.set(key, value);
    if (route.pathname.endsWith('.html')) merged.set('cache-control', 'no-store');
    // The .wasm.gz asset is already compressed; do not gzip its bytes a second time.
    return new Response(response.body, { status: response.status, headers: merged, encodeBody: merged.has('content-encoding') ? 'manual' : 'automatic' });
  },
};
