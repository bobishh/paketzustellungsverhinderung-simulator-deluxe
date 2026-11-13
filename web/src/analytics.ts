import { analyticsConfig } from './analytics-config';

type Counter = 'delivered' | 'photos' | 'rings' | 'markedVans';
type Progress = Record<Counter, number> & { special: boolean; ended: boolean };
type TelemetryEvent = {
  event_id: string; occurred_at: string; event: string; build: string;
  session_id: string; device_id: string; workspace_id: string; entity_type: string; entity_id: string;
  trace_id: string; span_id: string; parent_span_id: string;
  component: string; operation: string; status: string; duration_ms: number;
  attrs: { mode: 'courier' | 'dog'; object_count?: number };
};
const counterEvents: Record<Counter, string> = {
  delivered: 'game.delivery.completed', photos: 'game.delivery.photographed',
  rings: 'game.intercom.rang', markedVans: 'game.van.marked',
};
const clickEvents: Record<string, string> = {
  play: 'game.play.clicked', solo: 'game.shift.solo_requested', friends: 'game.shift.host_requested',
  share: 'game.invite.copy_requested', signin: 'game.identity.opened', 'player-profile': 'game.profile.opened',
};
const activeInstances = new WeakMap<Window, () => void>();

function progressSnapshot(detail: unknown): Progress | undefined {
  if (!detail || typeof detail !== 'object') return;
  const value = detail as Record<string, unknown>;
  const count = (key: Counter) => typeof value[key] === 'number' && Number.isSafeInteger(value[key])
    && value[key] >= 0 && value[key] <= 10_000;
  if (!(['delivered', 'photos', 'rings', 'markedVans'] as const).every(count)) return;
  // Explicit allowlist: never serialize the HUD, address, invitation, mesh or Identity payload.
  return { delivered: value.delivered as number, photos: value.photos as number,
    rings: value.rings as number, markedVans: value.markedVans as number,
    special: value.special === true, ended: value.ended === true };
}

/** Production-only, cookieless product counts; call after consuming the Identity callback. */
export function initGameAnalytics(options: { target?: Window; fetch?: typeof fetch; now?: () => number } = {}): () => void {
  const target = options.target ?? window;
  if (activeInstances.has(target)) return activeInstances.get(target)!;
  if (target.location.origin !== analyticsConfig.origin || new URLSearchParams(target.location.search).has('press-shot')
    || target.navigator.doNotTrack === '1' || (target.navigator as Navigator & { globalPrivacyControl?: boolean }).globalPrivacyControl) return () => {};
  const doc = target.document, fetcher = options.fetch ?? target.fetch.bind(target);
  const now = options.now ?? (() => target.performance.now());
  const removers: (() => void)[] = [];
  let mode: 'courier' | 'dog' = 'courier';
  let riding = false, ended = false, baseline: Progress | undefined, startedAt = 0;
  let queue: TelemetryEvent[] = [], stopped = false, windowStart = now(), windowCount = 0;
  let activeMs = 0, lastTick = now(), lastActivity = now(), reportedActive = false;
  const clickTimes = new Map<string, number>();
  const listen = (surface: EventTarget, type: string, fn: EventListener) => {
    surface.addEventListener(type, fn); removers.push(() => surface.removeEventListener(type, fn));
  };
  const send = (envelope: unknown) => {
    try {
      void fetcher(analyticsConfig.endpoint, { method: 'POST', credentials: 'omit', mode: 'cors',
        referrerPolicy: 'no-referrer', keepalive: true,
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${analyticsConfig.browserToken}` },
        body: JSON.stringify(envelope) }).catch(() => undefined);
    } catch { /* Analytics must never interrupt play, including synchronous transport failures. */ }
  };
  const webEvent = (event: 'page_view' | 'active_read') => send({ schema_version: 1, project: analyticsConfig.project,
    source: 'browser', events: [{ event_id: target.crypto.randomUUID(), occurred_at: new Date().toISOString(), event,
      path: '/', target: '', referrer_host: '', properties: event === 'active_read' ? { active_seconds: '30' } : {} }] });
  const flush = () => {
    if (!queue.length) return;
    const events = queue; queue = [];
    send({ schema_version: 2, project: analyticsConfig.project, stream: 'telemetry', source: 'browser', events });
  };
  const record = (event: string, objectCount?: number, duration = 0) => {
    if (stopped) return;
    if (now() - windowStart >= 60_000) { windowStart = now(); windowCount = 0; }
    if (windowCount >= 120) return;
    windowCount++;
    queue.push({ event_id: target.crypto.randomUUID(), occurred_at: new Date().toISOString(), event, build: '',
      session_id: '', device_id: '', workspace_id: '', entity_type: '', entity_id: '', trace_id: '', span_id: '', parent_span_id: '',
      component: 'game', operation: event.split('.')[1], status: 'ok', duration_ms: Math.min(86_400_000, Math.max(0, duration)),
      attrs: { mode, ...(objectCount === undefined ? {} : { object_count: objectCount }) } });
    if (queue.length >= 20) flush();
  };
  listen(doc, 'click', event => {
    const element = (event.target as Element | null)?.closest?.<HTMLElement>('button');
    if (!element || element.matches(':disabled')) return;
    if (element.dataset.role === 'courier' || element.dataset.role === 'dog') mode = element.dataset.role;
    const name = clickEvents[element.id];
    if (name && now() - (clickTimes.get(name) ?? -Infinity) >= 1000) {
      clickTimes.set(name, now()); record(name);
    }
  });
  listen(doc, 'submit', event => {
    const id = (event.target as HTMLFormElement | null)?.id;
    if (id === 'join-form') record('game.shift.join_requested');
    if (id === 'address-form') record('game.location.search_requested');
  });
  listen(target, 'paket-mode', event => {
    const next = (event as CustomEvent<unknown>).detail;
    if (next === 'ride' && !riding) {
      riding = true; ended = false; baseline = undefined; startedAt = now();
      mode = doc.querySelector<HTMLElement>('[data-role][aria-pressed="true"]')?.dataset.role === 'dog' ? 'dog' : 'courier';
      record('game.shift.started');
    } else if (next === 'map' && riding) {
      if (!ended) record('game.shift.left', undefined, now() - startedAt);
      riding = false; baseline = undefined; flush();
    }
  });
  listen(target, 'paket-progress', event => {
    if (!riding || ended) return;
    const value = progressSnapshot((event as CustomEvent<unknown>).detail);
    if (!value) return;
    if (baseline) {
      for (const key of Object.keys(counterEvents) as Counter[]) {
        const delta = value[key] - baseline[key];
        if (delta > 0) record(counterEvents[key], delta);
      }
      if (value.special && !baseline.special) record('game.delivery.bonus');
      if (value.ended && !baseline.ended) {
        ended = true; record('game.shift.completed', undefined, now() - startedAt); flush();
      }
    } else if (value.ended) ended = true;
    // First snapshot is a baseline: joining an existing shift is not a burst of new deliveries.
    baseline = value;
  });
  listen(target, 'paket-day', () => {
    if (!riding) return;
    if (!ended) record('game.shift.completed', undefined, now() - startedAt);
    ended = false; baseline = undefined; startedAt = now();
    record('game.shift.started'); flush();
  });
  for (const type of ['pointerdown', 'keydown', 'touchstart']) listen(doc, type, () => { lastActivity = now(); });
  listen(doc, 'visibilitychange', () => { lastTick = now(); if (doc.visibilityState === 'hidden') flush(); });
  listen(target, 'pagehide', flush);
  const timer = target.setInterval(() => {
    const time = now(), elapsed = Math.min(5000, Math.max(0, time - lastTick)); lastTick = time;
    if (!reportedActive && doc.visibilityState === 'visible' && doc.hasFocus() && time - lastActivity < 60_000) {
      activeMs += elapsed;
      if (activeMs >= 30_000) { reportedActive = true; webEvent('active_read'); }
    }
    flush();
  }, 5000);
  const dispose = () => {
    flush(); stopped = true; target.clearInterval(timer); removers.forEach(remove => remove()); activeInstances.delete(target);
  };
  activeInstances.set(target, dispose);
  webEvent('page_view');
  return dispose;
}
