const overlay = document.querySelector<HTMLElement>('#world-loading')!;
const label = document.querySelector<HTMLElement>('#world-loading-text')!;
const spinner = overlay.querySelector<HTMLElement>('.spinner')!;
const retry = document.querySelector<HTMLButtonElement>('#retry-map')!;
const choose = document.querySelector<HTMLButtonElement>('#choose-neighbourhood')!;
let mode = '';
let timeout: number | undefined;

function show(pending: boolean, text: string, failed = false, canChoose = false) {
  overlay.hidden = !pending && !failed && !canChoose;
  overlay.setAttribute('aria-busy', String(pending));
  spinner.hidden = !pending;
  label.textContent = text;
  retry.hidden = !failed;
  choose.hidden = !canChoose;
}

function startRide(text: string) {
  window.clearTimeout(timeout);
  show(true, text);
  timeout = window.setTimeout(() => {
    show(false, 'The neighbourhood is taking longer than expected.', true, true);
  }, 65_000);
}

export function showStartup() {
  if (!mode) show(true, 'Starting renderer…');
}

export function dismissStartup() {
  if (!mode) show(false, '');
}

window.addEventListener('paket-mode', event => {
  mode = (event as CustomEvent<string>).detail;
  overlay.classList.toggle('map-loading', mode === 'map');
  window.clearTimeout(timeout);
  if (mode === 'map') show(true, 'Loading map…');
  else startRide('Starting neighbourhood…');
});

window.addEventListener('paket-map-loading', event => {
  if (mode !== 'map') return;
  const state = (event as CustomEvent<{pending: boolean; failed: boolean}>).detail;
  show(state.pending, state.failed ? 'Some map tiles could not load.' : 'Loading map…', state.failed);
});

window.addEventListener('paket-ride-loading', event => {
  if (mode !== 'ride') return;
  const state = (event as CustomEvent<string>).detail;
  if (state === 'failed' || state === 'empty') {
    window.clearTimeout(timeout);
    show(false, state === 'failed' ? 'Neighbourhood could not load.' : 'No playable streets here.', state === 'failed', true);
  }
});

window.addEventListener('paket-hud', event => {
  if (mode !== 'ride' || !(event as CustomEvent<{ready: boolean}>).detail.ready) return;
  window.clearTimeout(timeout);
  show(false, '');
});

retry.onclick = () => {
  globalThis.paketCommands.push({action: 'retry-map'});
  if (mode === 'ride') startRide('Retrying neighbourhood…');
  else show(true, 'Loading map…');
};
choose.onclick = () => {
  globalThis.paketCommands.push({action: 'change-location'});
};
