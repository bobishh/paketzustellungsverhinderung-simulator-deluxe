const canvas = document.querySelector<HTMLCanvasElement>('#game')!;
const button = document.querySelector<HTMLButtonElement>('#capture-mouse')!;
let riding = false;
let interaction = false;
let pending = false;

const ownsLock = () => document.pointerLockElement === canvas;
const canCapture = () => riding && !interaction && !document.querySelector('dialog[open]');
const update = () => { button.hidden = !riding || interaction || ownsLock(); };

function failed() {
  if (!pending) return;
  pending = false;
  if (riding && !ownsLock()) {
    paketNotify('Mouse capture was blocked. Click Capture mouse to try again.');
  }
}

function capture() {
  if (!canCapture() || pending || ownsLock()) return;
  canvas.focus({preventScroll: true});
  pending = true;
  try {
    void canvas.requestPointerLock()?.catch(failed);
  } catch {
    failed();
  }
}

canvas.addEventListener('click', capture);
button.addEventListener('click', capture);
document.addEventListener('pointerlockerror', failed);
document.addEventListener('pointerlockchange', () => {
  pending = false;
  if (ownsLock() && !canCapture()) {
    document.exitPointerLock();
    return;
  }
  update();
  if (riding) paketCommands.push({action: 'look', active: ownsLock()});
});
window.addEventListener('paket-mode', event => {
  riding = (event as CustomEvent<string>).detail === 'ride';
  interaction = false;
  if (!riding && ownsLock()) document.exitPointerLock();
  update();
});
window.addEventListener('paket-hud', event => {
  const active = Boolean((event as CustomEvent<{interaction?: boolean}>).detail.interaction);
  if (interaction === active) return;
  interaction = active;
  update();
});
