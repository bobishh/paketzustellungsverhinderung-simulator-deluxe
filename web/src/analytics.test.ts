import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { initGameAnalytics } from './analytics';
import { analyticsConfig } from './analytics-config';

function fixture(origin = analyticsConfig.origin as string, search = '') {
  const doc = Object.assign(new EventTarget(), {
    visibilityState: 'visible', hasFocus: () => true,
    querySelector: () => ({ dataset: { role: 'courier' } }),
  });
  const target = Object.assign(new EventTarget(), {
    document: doc, location: { origin, search, pathname: '/identity/return', hash: '#join=SECRET_INVITE&identity=SECRET_KEY' },
    navigator: { doNotTrack: '0' }, crypto: globalThis.crypto,
    performance: { now: () => Date.now() },
    setInterval: globalThis.setInterval, clearInterval: globalThis.clearInterval,
  });
  const fetcher = vi.fn<typeof fetch>().mockResolvedValue(new Response('{"accepted":1}'));
  const emit = (type: string, detail: unknown) => target.dispatchEvent(new CustomEvent(type, { detail }));
  const requests = () => fetcher.mock.calls.map(([, options]) => JSON.parse(options!.body as string));
  const telemetry = () => requests().filter(x => x.schema_version === 2).flatMap(x => x.events);
  return { doc, target, fetcher, emit, requests, telemetry,
    init: () => initGameAnalytics({ target: target as unknown as Window, fetch: fetcher, now: () => Date.now() }) };
}
const emptyProgress = { delivered: 0, photos: 0, rings: 0, markedVans: 0, special: false, ended: false };

describe('game analytics privacy and transitions', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => { vi.clearAllTimers(); vi.useRealTimers(); });

  it('sends only allowlisted counters despite private URL and 10 Hz HUD payloads', () => {
    const f = fixture(analyticsConfig.origin, '?lat=52.49012&lon=13.39999&identity=SECRET_QUERY');
    const stop = f.init();
    f.emit('paket-mode', 'ride');
    f.emit('paket-progress', emptyProgress);
    for (let frame = 0; frame < 100; frame++) {
      f.emit('paket-hud', { routes: [{ name: 'PRIVATE_NAME', address: 'PRIVATE_ADDRESS', distance: 17 }], selected: 2 });
      f.emit('paket-radar', { position: [52.49012, 13.39999] });
      f.emit('paket-progress', { ...emptyProgress, delivered: 2, photos: 1, rings: 3,
        personId: 'PRIVATE_PERSON', invitation: 'PRIVATE_LINK', address: 'PRIVATE_ADDRESS', workMs: 999, score: 500 });
    }
    stop();
    expect(f.telemetry().map(e => [e.event, e.attrs.object_count])).toEqual([
      ['game.shift.started', undefined], ['game.delivery.completed', 2],
      ['game.delivery.photographed', 1], ['game.intercom.rang', 3],
    ]);
    expect(JSON.stringify(f.requests())).not.toMatch(/PRIVATE_|SECRET_|52\.49012|13\.39999|identity\/return/);
    expect(f.requests()[0].events[0].path).toBe('/');
    for (const [url, request] of f.fetcher.mock.calls) {
      expect(url).toBe(analyticsConfig.endpoint);
      expect(request).toMatchObject({ credentials: 'omit', referrerPolicy: 'no-referrer', keepalive: true, mode: 'cors' });
    }
  });

  it('baselines late joins, ignores invalid data and emits one end per shift', () => {
    const f = fixture(), stop = f.init();
    f.emit('paket-mode', 'ride'); f.emit('paket-mode', 'ride');
    f.emit('paket-progress', { ...emptyProgress, delivered: 8, rings: 12, photos: 6 });
    f.emit('paket-progress', { ...emptyProgress, delivered: '9' });
    f.emit('paket-progress', { ...emptyProgress, delivered: Infinity });
    f.emit('paket-progress', { ...emptyProgress, delivered: 9, rings: 12, photos: 6, ended: true });
    f.emit('paket-progress', { ...emptyProgress, delivered: 9, ended: true });
    f.emit('paket-mode', 'map');
    f.emit('paket-mode', 'ride');
    f.emit('paket-progress', emptyProgress);
    f.emit('paket-progress', { ...emptyProgress, delivered: 1 });
    f.emit('paket-mode', 'map'); stop();
    const events = f.telemetry();
    expect(events.filter(e => e.event === 'game.shift.started')).toHaveLength(2);
    expect(events.filter(e => e.event === 'game.shift.completed')).toHaveLength(1);
    expect(events.filter(e => e.event === 'game.shift.left')).toHaveLength(1);
    expect(events.filter(e => e.event === 'game.delivery.completed').map(e => e.attrs.object_count)).toEqual([1, 1]);
    expect(events.some(e => e.event === 'game.intercom.rang')).toBe(false);
  });

  it('tracks UI actions without text, entered addresses or invite links', () => {
    const f = fixture(), stop = f.init();
    const dispatch = (type: string, target: unknown) => {
      const event = new Event(type); Object.defineProperty(event, 'target', { value: target }); f.doc.dispatchEvent(event);
    };
    const profile = { id: 'player-profile', dataset: {}, matches: () => false, textContent: 'PRIVATE_PLAYER' };
    dispatch('click', { closest: () => profile }); dispatch('click', { closest: () => profile });
    dispatch('submit', { id: 'address-form', elements: [{ value: 'PRIVATE_ADDRESS' }] });
    dispatch('submit', { id: 'join-form', elements: [{ value: 'PRIVATE_INVITE' }] });
    stop();
    expect(f.telemetry().map(e => e.event)).toEqual(['game.profile.opened', 'game.location.search_requested', 'game.shift.join_requested']);
    expect(JSON.stringify(f.requests())).not.toContain('PRIVATE_');
  });

  it('suppresses preview, screenshots, Do Not Track and GPC sessions', () => {
    for (const f of [fixture('http://localhost:4188'), fixture('https://preview.workers.dev'), fixture(analyticsConfig.origin, '?press-shot')]) {
      f.init(); f.emit('paket-mode', 'ride'); vi.advanceTimersByTime(60_000); expect(f.fetcher).not.toHaveBeenCalled();
    }
    for (const preference of [{ doNotTrack: '1' }, { globalPrivacyControl: true }]) {
      const f = fixture(); Object.assign(f.target.navigator, preference); f.init(); expect(f.fetcher).not.toHaveBeenCalled();
    }
  });

  it('counts visible active time once and does not duplicate collectors', () => {
    const f = fixture(), stop = f.init();
    expect(f.init()).toBe(stop);
    f.doc.visibilityState = 'hidden'; vi.advanceTimersByTime(60_000);
    expect(f.requests()).toHaveLength(1);
    f.doc.visibilityState = 'visible'; f.doc.dispatchEvent(new Event('pointerdown'));
    vi.advanceTimersByTime(30_000); vi.advanceTimersByTime(30_000);
    expect(f.requests().flatMap(x => x.events).map(x => x.event)).toEqual(['page_view', 'active_read']);
    stop(); f.emit('paket-mode', 'ride'); vi.advanceTimersByTime(60_000);
    expect(f.requests()).toHaveLength(2);
  });

  it('flushes on page hide and treats network errors as optional', async () => {
    const f = fixture(); f.fetcher.mockRejectedValue(new Error('offline'));
    const stop = f.init(); f.emit('paket-mode', 'ride');
    expect(() => f.target.dispatchEvent(new Event('pagehide'))).not.toThrow();
    expect(f.telemetry()).toHaveLength(1);
    await Promise.resolve();
    f.fetcher.mockImplementation(() => { throw new Error('blocked'); });
    expect(() => { f.emit('paket-mode', 'map'); stop(); }).not.toThrow();
  });
});
