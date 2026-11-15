import './style.css';
import './lobby';
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
const gameArea = canvas.parentElement!;
const scenePixelBudget = 320_000;
let pixelBudget = scenePixelBudget;
let riding = false;
let lastPose = 0;
let slowPoses = 0;

function resizeScene() {
  if (!riding || document.body.classList.contains('press-shot')) return;
  const area = gameArea.clientWidth * gameArea.clientHeight;
  const scale = Math.min(1, Math.sqrt(pixelBudget / (area * devicePixelRatio ** 2)));
  // Resize the canvas content box for the GPU and scale its presentation back
  // to the full game area. The HTML HUD uses the browser's normal resolution.
  canvas.style.setProperty('--scene-size', `${scale * 100}%`);
  canvas.style.setProperty('--scene-upscale', String(1 / scale));
}
new ResizeObserver(resizeScene).observe(gameArea);
window.addEventListener('resize', resizeScene);
window.addEventListener('paket-radar', () => {
  if (!riding || pixelBudget < scenePixelBudget || document.hidden) return;
  const now = performance.now();
  slowPoses = lastPose && now - lastPose > 170 ? slowPoses + 1 : 0;
  lastPose = now;
  if (slowPoses === 4) {
    pixelBudget = 160_000;
    resizeScene();
  }
});
window.addEventListener('paket-mode', ((event: CustomEvent<string>) => {
  riding = event.detail === 'ride';
  lastPose = 0;
  slowPoses = 0;
  document.querySelector<HTMLElement>('#signin')!.hidden = riding;
  canvas.classList.toggle('bounded-scene', riding && !document.body.classList.contains('press-shot'));
  resizeScene();
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
    const gpu = (navigator as Navigator & { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
    if (!gpu || !await gpu.requestAdapter()) throw new Error('WebGPU is unavailable. Open this game in a current Chrome, Edge or Safari with graphics acceleration enabled.');
    await ready;
    const join = new URLSearchParams(location.hash.slice(1)).get('join');
    const query = new URLSearchParams(location.search);
    const lat = Number(query.get('lat')), lon = Number(query.get('lon'));
    globalThis.paketArgs = join ? ['--join', `${location.origin}/#join=${join}`] :
      query.has('lat') && query.has('lon') && Number.isFinite(lat) && Math.abs(lat) <= 85 && Number.isFinite(lon) && Math.abs(lon) <= 180 ? ['--lat', String(lat), '--lon', String(lon)] : [];
    if (query.has('press-shot')) { globalThis.paketArgs.push('--press-shot'); document.body.classList.add('press-shot'); }
    status.textContent = 'Loading the neighbourhood simulator…';
    const url = '/game/paket_game.js';
    const { default: init } = await import(/* @vite-ignore */ url);
    await init();
    showStartup();
    running = true;
    document.querySelector<HTMLElement>('#start')!.hidden = true;
    canvas.focus();
    paketNotify('Click the game to look around · Esc releases the cursor');
  } catch (error) {
    status.textContent = error instanceof Error ? error.message : String(error);
    button.disabled = false;
  } finally { document.querySelector<HTMLElement>('#start-spinner')!.hidden=true; }
});
