import { requestNeighbourhood } from './geolocation';

type Command = { action: string; [key: string]: unknown };
declare global { var paketCommands: Command[]; }
globalThis.paketCommands = [];
const send = (command: Command) => { paketCommands.push(command); };
const cities = [
  ['Berlin', 'Kreuzberg', 52.4895, 13.3908],
  ['New York', 'East Village', 40.7265, -73.9815],
  ['São Paulo', 'Vila Madalena', -23.5505, -46.6892],
  ['Tokyo', 'Shimokitazawa', 35.6616, 139.6682],
  ['Cape Town', 'Gardens', -33.9335, 18.4112],
  ['Sydney', 'Surry Hills', -33.8845, 151.2133],
] as const;
const status = document.querySelector<HTMLElement>('#lobby-status')!;
const lobby = document.querySelector<HTMLElement>('#lobby')!;
const results = document.querySelector('#address-results')!;
const selected = document.querySelector('#selected-place')!;
const locationButton = document.querySelector<HTMLButtonElement>('#my-location')!;
const locationStatus = document.querySelector<HTMLElement>('#location-status')!;
let locationRequest = 0, locating = false, lobbyPending = false;
function cancelLocation() {
  locationRequest++; locating = false;
  locationButton.disabled = lobbyPending;
  locationButton.removeAttribute('aria-busy');
  locationButton.querySelector('span')!.textContent = 'My location';
  locationStatus.hidden = true;
}
locationButton.addEventListener('click', async () => {
  if (locating || lobbyPending) return;
  const request = ++locationRequest;
  locating = true; locationButton.disabled = true;
  locationButton.setAttribute('aria-busy', 'true');
  locationButton.querySelector('span')!.textContent = 'Locating…';
  locationStatus.hidden = false;
  locationStatus.textContent = 'Allow location in your browser to find your neighbourhood.';
  try {
    const point = await requestNeighbourhood(navigator.geolocation);
    if (request !== locationRequest || lobby.hidden) return;
    select(point.lat, point.lon, 'Your neighbourhood');
    results.replaceChildren();
    locationStatus.textContent = 'Neighbourhood selected. Drag the map to fine-tune your start.';
    locationStatus.hidden = false;
  } catch (error) {
    if (request !== locationRequest || lobby.hidden) return;
    cancelLocation();
    locationStatus.textContent = error instanceof Error ? error.message : 'Location unavailable. Search for an address instead.';
    locationStatus.hidden = false;
  }
});
// A late permission response must not overwrite a newer selection or start a ride elsewhere.
document.querySelector('#game')!.addEventListener('pointerdown', () => { if (!lobby.hidden && locating) cancelLocation(); });
let chosen: { lat: number; lon: number; name: string } | undefined;
let autoStart = false, sawPending = false, inviteLink = '';
function select(lat: number, lon: number, name: string) {
  cancelLocation();
  chosen = { lat, lon, name }; selected.textContent = name;
  send({ action: 'select', lat, lon });
  document.querySelectorAll<HTMLButtonElement>('.city-grid button').forEach(b => b.setAttribute('aria-pressed', String(b.dataset.city === name)));
}
for (const [name, district, lat, lon] of cities) {
  const button = document.createElement('button'); button.textContent = name;
  button.dataset.city = `${name} · ${district}`; button.setAttribute('aria-pressed', String(name === 'Berlin'));
  button.onclick = () => { select(lat, lon, button.dataset.city!); results.replaceChildren(); };
  document.querySelector('.city-grid')!.append(button);
}
document.querySelectorAll<HTMLButtonElement>('[data-role]').forEach(button => button.onclick = () => {
  send({ action: 'role', role: button.dataset.role });
  document.querySelectorAll('[data-role]').forEach(b => b.setAttribute('aria-pressed', String(b === button)));
});
document.querySelectorAll<HTMLButtonElement>('[data-command]').forEach(b => b.onclick = () => send({ action: b.dataset.command! }));
document.querySelector('#solo')!.addEventListener('click', () => { cancelLocation(); send({ action: 'solo' }); });
document.querySelector('#friends')!.addEventListener('click', () => { cancelLocation(); autoStart = true; sawPending = false; send({ action: 'new' }); status.textContent = 'Opening your shift…'; });
document.querySelector('#join-form')!.addEventListener('submit', e => {
  e.preventDefault(); cancelLocation(); autoStart = true; sawPending = false;
  send({ action: 'join', link: document.querySelector<HTMLInputElement>('#invite')!.value.trim() });
});
document.querySelector('#share')!.addEventListener('click', () => { void navigator.clipboard.writeText(inviteLink).then(() => paketNotify('Invite copied. Send it to a friend.')).catch(() => { paketNotify('Select and copy the invite link.'); window.prompt('Invite link', inviteLink); }); });
window.addEventListener('paket-mode', ((event: CustomEvent<string>) => {
  lobby.hidden = event.detail !== 'map';
  if (lobby.hidden) cancelLocation();
  document.querySelector<HTMLElement>('#minimap')!.hidden = event.detail !== 'ride';
  if (event.detail === 'ride') { document.querySelector<HTMLCanvasElement>('#game')!.focus(); }
}) as EventListener);
window.addEventListener('paket-lobby', ((event: CustomEvent<any>) => {
  const s = event.detail;
  lobbyPending = Boolean(s.pending);
  if (lobbyPending && locating) cancelLocation();
  locationButton.disabled = lobbyPending || locating;
  selected.textContent = chosen && Math.abs(chosen.lat - s.lat) < 1e-6 && Math.abs(chosen.lon - s.lon) < 1e-6 ? chosen.name : s.label;
  document.querySelectorAll<HTMLButtonElement>('.city-grid button').forEach((button, index) => {
    const city = cities[index];
    button.setAttribute('aria-pressed', String(Math.abs(city[2] - s.lat) < 1e-5 && Math.abs(city[3] - s.lon) < 1e-5));
  });
  if (s.status) status.textContent = s.status;
  if (s.pending) sawPending = true;
  document.querySelectorAll<HTMLButtonElement>('#solo,#friends,#join-form button').forEach(b => b.disabled = s.pending);
  if (s.link) { inviteLink = s.link; document.querySelector<HTMLElement>('#share')!.hidden = false; }
  if (autoStart && sawPending && s.ready) { autoStart = false; send({ action: 'start' }); }
  if (autoStart && sawPending && !s.pending && !s.ready) autoStart = false;
}) as EventListener);
let searching = false, searchedAt = 0;
document.querySelector('#address-form')!.addEventListener('submit', async e => {
  e.preventDefault(); if (searching || Date.now() - searchedAt < 1200) return;
  const q = document.querySelector<HTMLInputElement>('#address')!.value.trim(); if (q.length < 3) return;
  searching = true; searchedAt = Date.now(); results.textContent = 'Finding that address…';
  const button = document.querySelector<HTMLButtonElement>('#address-form button')!; button.disabled = true;
  try {
    const r = await fetch(`/api/address?q=${encodeURIComponent(q)}`, { signal: AbortSignal.timeout(15000) });
    let data: { name: string; lat: number; lon: number }[];
    if(r.ok) data=await r.json(); else {
      const fallback=await fetch(`https://photon.komoot.io/api/?q=${encodeURIComponent(q)}&limit=5`,{signal:AbortSignal.timeout(12000)});
      if(!fallback.ok)throw new Error('Address search is busy. Try again or choose a city.');
      const body=await fallback.json();
      data=(body.features??[]).filter((f:any)=>f.geometry?.type==='Point'&&f.geometry.coordinates.every(Number.isFinite)).slice(0,5).map((f:any)=>({lat:f.geometry.coordinates[1],lon:f.geometry.coordinates[0],name:[f.properties.name,[f.properties.street,f.properties.housenumber].filter(Boolean).join(' '),f.properties.city||f.properties.district,f.properties.country].filter(Boolean).join(', ')}));
    }
    results.replaceChildren();
    if (!data.length) results.textContent = 'No results. Add a city or postal code.';
    for (const item of data) {
      const b = document.createElement('button'); b.textContent = item.name;
      b.onclick = () => { select(item.lat, item.lon, item.name); results.replaceChildren(); };
      results.append(b);
    }
  } catch (error) { results.textContent = error instanceof Error ? error.message : 'Search unavailable'; }
  finally { searching = false; button.disabled = false; }
});

export {};
