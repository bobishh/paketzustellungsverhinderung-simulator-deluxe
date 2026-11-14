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
    `[out:json][timeout:25];(way["building"](${bbox});way["highway"](${bbox});nwr["leisure"="playground"](${bbox});node["amenity"="bench"](${bbox});node["natural"="tree"](${bbox}););out geom;`;
  return { query, file: `${lat.toFixed(5)}_${lon.toFixed(5)}_${landscape ? 'landscape' : `${x}_${y}`}.json` };
}
async function directory(request, env) {
  const reply = (body, status = 200) => Response.json(body, {status, headers:{...headers,'cache-control':'no-store'}});
  if (request.method === 'GET') {
    const partitions = await Promise.all([0,1,2,3].map(i => env.MESH_DIRECTORY.getByName(`public-${i}`).list()));
    return reply(partitions.flat());
  }
  if (!['PUT','DELETE'].includes(request.method)) return reply({error:'Method not allowed'},405);
  if (request.headers.get('origin') !== new URL(request.url).origin) return reply({error:'Invalid origin'},403);
  const token = request.headers.get('authorization')?.replace(/^Bearer /,'');
  if (!token || !/^[a-f0-9]{64}$/.test(token)) return reply({error:'Invalid owner'},403);
  // Bound the stream even when Content-Length is absent or dishonest.
  const reader = request.body?.getReader();
  if (!reader) return reply({error:'Missing invitation'},400);
  const chunks = []; let size = 0;
  try {
    while (true) {
      const {done,value} = await reader.read(); if (done) break;
      size += value.length;
      if (size > 4096) { await reader.cancel(); return reply({error:'Invitation too large'},413); }
      chunks.push(value);
    }
    const bytes = new Uint8Array(size); let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk,offset); offset += chunk.length; }
    const input = JSON.parse(new TextDecoder().decode(bytes));
    const room = [...new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(token)))].map(x=>x.toString(16).padStart(2,'0')).join('');
    if (input.room !== room) return reply({error:'Invalid owner'},403);
    const target = env.MESH_DIRECTORY.getByName(`public-${parseInt(room[0],16)%4}`);
    if (request.method === 'DELETE') { await target.remove(room); return reply({ok:true}); }
    if (input.protocol !== 'paket-web/1' || !/^[a-f0-9]{64}$/.test(input.host) ||
      !Number.isFinite(input.origin?.lat) || Math.abs(input.origin.lat)>85 || !Number.isFinite(input.origin?.lon) || Math.abs(input.origin.lon)>180 ||
      typeof input.relay !== 'string' || input.relay.length>512 || new URL(input.relay).protocol !== 'https:') return reply({error:'Invalid invitation'},400);
    const invite = {protocol:'paket-web/1',room,host:input.host,relay:input.relay,origin:{lat:input.origin.lat,lon:input.origin.lon}};
    return await target.publish(invite) ? reply({ok:true}) : reply({error:'Directory is full'},503);
  } catch (error) {
    if (error instanceof SyntaxError || error instanceof TypeError) return reply({error:'Invalid invitation'},400);
    throw error;
  }
}
export default {
  async fetch(request, env, ctx) {
    if (new URL(request.url).pathname === '/api/meshes') {
      try { return await directory(request, env); }
      catch { return Response.json({error:'Mesh list unavailable'}, {status:503,headers:{...headers,'cache-control':'no-store'}}); }
    }
    if (!['GET', 'HEAD'].includes(request.method)) return new Response('Method not allowed', { status: 405, headers: { ...headers, allow: 'GET, HEAD' } });
    const url = new URL(request.url);
    if (url.pathname === '/api/address') {
      const q = (url.searchParams.get('q') ?? '').trim();
      if (q.length < 3 || q.length > 180) return new Response('Invalid address', { status: 400, headers });
      const cacheKey = new Request(new URL('/address-cache?q=' + encodeURIComponent(q.toLowerCase()), url));
      const cached = await caches.default.match(cacheKey); if (cached) return cached;
      try {
        // Submit-only search, fixed upstream, cached results. No keystroke traffic.
        const upstream = new URL('/api/', env.PHOTON_ORIGIN || 'https://photon.komoot.io');
        upstream.search = new URLSearchParams({q, limit:'5'}).toString();
        const result = await fetch(upstream, {headers:{'user-agent':'Paketzustellungsverhinderung Simulator Deluxe / address selection'},signal:AbortSignal.timeout(12000)});
        if (!result.ok) throw new Error('Search unavailable');
        const data = await result.json();
        const places = (data.features ?? []).slice(0,5).filter(f => f.geometry?.type==='Point' && f.geometry.coordinates.every(Number.isFinite)).map(f => {
          const p=f.properties; return {lat:f.geometry.coordinates[1],lon:f.geometry.coordinates[0],name:[p.name,[p.street,p.housenumber].filter(Boolean).join(' '),p.postcode,p.city||p.town||p.district,p.country].filter(Boolean).filter((v,i,a)=>a.indexOf(v)===i).join(',')};
        });
        const response = Response.json(places,{headers:{...headers,'cache-control':'public,max-age=86400'}});
        ctx.waitUntil(caches.default.put(cacheKey,response.clone()));return response;
      } catch {return new Response('Address search unavailable',{status:503,headers});}
    }
    if (url.pathname === '/api/osm') {
      let map;
      try { map = mapRequest(url); } catch { return new Response('Invalid map request', { status: 400, headers }); }
      const assetUrl = new URL(`/osm/${map.file}`, url);
      const bundled = await env.ASSETS.fetch(new Request(assetUrl));
      if (bundled.ok) return new Response(request.method === 'HEAD' ? null : bundled.body, { headers: { ...headers, 'content-type': 'application/json', 'cache-control': 'public,max-age=86400' } });
      const cacheKey = new Request(new URL(`/map-cache/v2/${map.file}`, url));
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
      if (asset.ok || asset.status === 304) {
        const encoded = new Headers(asset.headers);
        encoded.set('content-type', 'application/wasm'); encoded.set('content-encoding', 'gzip');
        response = new Response(asset.body, { status: asset.status, headers: encoded });
      }
    }
    const merged = new Headers(response.headers);
    for (const [key, value] of Object.entries(headers)) merged.set(key, value);
    if (response.status === 404 || route.pathname.endsWith('.html')) merged.set('cache-control', 'no-store');
    // The .wasm.gz asset is already compressed; do not gzip its bytes a second time.
    return new Response(response.body, { status: response.status, headers: merged, encodeBody: merged.has('content-encoding') ? 'manual' : 'automatic' });
  },
};
