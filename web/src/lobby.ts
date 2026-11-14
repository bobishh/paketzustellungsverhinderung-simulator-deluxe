import { availableMeshes } from './mesh-directory';
import { requestNeighbourhood } from './geolocation';

type Command = { action: string; [key: string]: unknown };
declare global { var paketCommands: Command[]; var paketPublicMesh: boolean; }
globalThis.paketCommands = [];
globalThis.paketPublicMesh = false;
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
document.querySelector('#friends')!.addEventListener('click', () => { cancelLocation(); globalThis.paketPublicMesh = document.querySelector<HTMLInputElement>('#public-mesh')!.checked; autoStart = true; sawPending = false; send({ action: 'new' }); status.textContent = 'Opening your shift…'; });
document.querySelector('#join-form')!.addEventListener('submit', e => {
  e.preventDefault(); cancelLocation(); autoStart = true; sawPending = false;
  send({ action: 'join', link: document.querySelector<HTMLInputElement>('#invite')!.value.trim() });
});
const share = document.querySelector<HTMLButtonElement>('#share')!;
const shareDialog = document.querySelector<HTMLDialogElement>('#share-dialog')!;
const shareLink = document.querySelector<HTMLInputElement>('#share-link')!;
const shareStatus = document.querySelector<HTMLElement>('#share-status')!;
function setInvitation(link: string) {
  inviteLink = link; share.disabled = !link; shareLink.value = link;
  if (!link && shareDialog.open) shareDialog.close();
}
share.addEventListener('click', () => {
  if (!inviteLink) return;
  document.exitPointerLock();
  send({action: 'menu', open: true});
  shareStatus.textContent = '';
  shareDialog.showModal(); shareLink.select();
});
document.querySelector('#share-close')!.addEventListener('click', () => shareDialog.close());
shareDialog.addEventListener('close', () => send({action: 'menu', open: false}));
document.querySelector('#copy-invite')!.addEventListener('click', async () => {
  try { await navigator.clipboard.writeText(inviteLink); shareStatus.textContent = 'Copied. Send the invitation to your friend.'; }
  catch { shareLink.focus(); shareLink.select(); shareStatus.textContent = 'Select the link and copy it with Ctrl+C or ⌘C.'; }
});
window.addEventListener('paket-session', ((event: CustomEvent<{link: string} | null>) => {
  setInvitation(event.detail?.link ?? '');
}) as EventListener);
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
  document.querySelectorAll<HTMLButtonElement>('#solo,#friends,#join-form button,#mesh-list button').forEach(b => b.disabled = s.pending);
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

const meshList = document.querySelector<HTMLElement>('#mesh-list')!;
const meshStatus = document.querySelector<HTMLElement>('#mesh-list-status')!;
const refreshMeshes = document.querySelector<HTMLButtonElement>('#refresh-meshes')!;
async function refresh() {
  if (refreshMeshes.disabled) return;
  refreshMeshes.disabled = true; meshStatus.textContent = 'Looking for public meshes…';
  try {
    const meshes = await availableMeshes(); meshList.replaceChildren();
    meshStatus.textContent = meshes.length ? `${meshes.length} public meshes. Choose one to join.` : 'No public meshes yet. Create one or paste a friend’s invitation.';
    for (const invite of meshes) {
      const button = document.createElement('button');
      const city = cities.find(c => Math.abs(c[2]-invite.origin.lat)<0.03 && Math.abs(c[3]-invite.origin.lon)<0.03);
      button.textContent = `${city ? city[0] : `${invite.origin.lat.toFixed(3)}, ${invite.origin.lon.toFixed(3)}`} · ${invite.room.slice(0,6)} · Join`;
      button.disabled = lobbyPending;
      button.onclick = () => {
        if (lobbyPending) return;
        cancelLocation(); autoStart = true; sawPending = false;
        send({action:'join',link:JSON.stringify(invite)});
      };
      meshList.append(button);
    }
  } catch (error) { meshList.replaceChildren(); meshStatus.textContent = error instanceof Error ? error.message : 'Mesh list unavailable.'; }
  finally { refreshMeshes.disabled = false; }
}
refreshMeshes.onclick = () => { void refresh(); };
window.addEventListener('paket-mode', ((event: CustomEvent<string>) => {
  if (event.detail === 'map') { void refresh(); }
}) as EventListener);
