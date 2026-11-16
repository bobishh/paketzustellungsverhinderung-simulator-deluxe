import './style.css';
import './basemap';
import {openInvitation} from './lobby';
import {setSoftwareRendering} from './render-quality';
import {showStartup} from './loading';
import './minimap';
import './hud';
import './shift-vote';
import './help';
import './bite-audio';
import { initGameAnalytics } from './analytics';
import { initProfile } from './profile';
import { restoreIdentity, signIn } from './identity';
import { mesh } from './mesh';

declare global { var paketArgs: string[]; var paketMesh: typeof mesh; var paketPhoto: (bytes: Uint8Array, index: number) => void; var paketNotify: (text: string) => void; var paketWantsPointer: boolean; }
// Consume identity evidence before loading game assets or making network requests.
const callbackHash = location.pathname === '/identity/return' ? location.hash : null;
if (callbackHash !== null) history.replaceState(null, '', '/identity/return');
initGameAnalytics();
const status = document.querySelector('#loading')!;
const notice = document.querySelector<HTMLElement>('#notice')!;
let noticeTimer: ReturnType<typeof setTimeout>;
globalThis.paketNotify = text => { notice.textContent = text; notice.hidden = false; clearTimeout(noticeTimer); noticeTimer = setTimeout(() => { notice.hidden = true; }, 7000); };
globalThis.paketMesh = mesh;
let photoUrl: string | undefined;
let photoName = '';
document.querySelector('#photo')!.addEventListener('click', () => {
  if (!photoUrl) return;
  const download = document.createElement('a');
  download.href = photoUrl; download.download = photoName; download.click();
});
const canvas = document.querySelector<HTMLCanvasElement>('#game')!;
let showedLookHint = false;
window.addEventListener('paket-mode', ((event: CustomEvent<string>) => {
  document.querySelector<HTMLElement>('#profile-identity')!.hidden = event.detail === 'ride';
  if (event.detail === 'ride' && !showedLookHint && matchMedia('(pointer: fine)').matches) {
    showedLookHint = true;
    paketNotify('Click the game to look around · Esc releases the cursor');
  }
}) as EventListener);
globalThis.paketPhoto = (bytes, index) => {
  if (photoUrl) URL.revokeObjectURL(photoUrl);
  photoUrl = URL.createObjectURL(new Blob([bytes.slice().buffer], { type: 'image/png' }));
  photoName = `delivery-${index + 1}-${Date.now()}.png`;
  document.querySelector<HTMLElement>('#photo')!.hidden = false;
  paketNotify('Photo ready. Download it using the button above.');
};
const ready = restoreIdentity(callbackHash).then(async path => { if (path && path.startsWith('/') && !path.startsWith('//')) history.replaceState(null, '', path); await initProfile(); })
  .catch(error => { paketNotify(`Identity: ${error.message}`); throw error; });
void ready.catch(() => undefined);
document.querySelector('#signin')!.addEventListener('click', () => { void signIn().catch(error => paketNotify(error.message)); });
document.querySelector('#fullscreen')!.addEventListener('click', () => { void (document.fullscreenElement ? document.exitFullscreen() : document.documentElement.requestFullscreen()).catch(error => paketNotify(error.message)); });
let running = false;
canvas.addEventListener('pointerdown', event => {
  if (running && globalThis.paketWantsPointer && !document.pointerLockElement) {
    void (event.currentTarget as HTMLCanvasElement).requestPointerLock()?.catch(() => paketNotify('Click the game to capture the mouse. Esc releases it.'));
  }
});
document.querySelector('#play')!.addEventListener('click', async () => {
  if (running) return;
  const button = document.querySelector<HTMLButtonElement>('#play')!;
  button.disabled = true;
  document.querySelector<HTMLElement>('#start-spinner')!.hidden=false;
  try {
    const gpu = (navigator as Navigator & { gpu?: { requestAdapter(): Promise<{info?:{isFallbackAdapter?:boolean}}|null> } }).gpu;
    const adapter = await gpu?.requestAdapter();
    if (!adapter) throw new Error('WebGPU is unavailable. Open this game in a current Chrome, Edge or Safari with graphics acceleration enabled.');
    setSoftwareRendering(Boolean(adapter.info?.isFallbackAdapter));
    await ready;
    const join = new URLSearchParams(location.hash.slice(1)).get('join');
    const query = new URLSearchParams(location.search);
    const lat = Number(query.get('lat')), lon = Number(query.get('lon'));
    if(join)openInvitation(`${location.origin}/#join=${join}`);
    globalThis.paketArgs = !join && query.has('lat') && query.has('lon') && Number.isFinite(lat) && Math.abs(lat) <= 85 && Number.isFinite(lon) && Math.abs(lon) <= 180 ? ['--lat', String(lat), '--lon', String(lon)] : [];
    if (query.has('press-shot')) { globalThis.paketArgs.push('--press-shot'); document.body.classList.add('press-shot'); }
    if (query.has('inspect-courier')) globalThis.paketArgs.push('--inspect-courier');
    status.textContent = 'Loading the neighbourhood simulator…';
    const url = '/game/paket_game.js';
    const { default: init } = await import(/* @vite-ignore */ url);
    await init();
    showStartup();
    running = true;
    document.querySelector<HTMLElement>('#start')!.hidden = true;
    canvas.focus();
  } catch (error) {
    status.textContent = error instanceof Error ? error.message : String(error);
    button.disabled = false;
  } finally { document.querySelector<HTMLElement>('#start-spinner')!.hidden=true; }
});
