import {expect, test} from '@playwright/test';
import {clickUi, fact, observeGame, waitRide} from './bevy-ui';

test.beforeEach(async ({page}) => {
  await observeGame(page);
  await page.route('**/elevation/**', r => r.fulfill({status:503}));
  await page.route('**/api/meshes', r => r.fulfill({json:[]}));
  await page.route('**/api/osm?*', r => {
    const q = new URL(r.request().url()).searchParams;
    return q.get('x') === '0' && q.get('y') === '0' && !q.has('landscape')
      ? r.fulfill({path:new URL('../../fixtures/kreuzberg.json', import.meta.url).pathname, contentType:'application/json'})
      : r.fulfill({json:{elements:[]}});
  });
});

test('Given unavailable welcome artwork, when boot finishes, Play still opens the Bevy lobby', async ({page}) => {
  await page.route('**/assets/landing/**', r => r.fulfill({status:404}));
  await page.goto('/');
  await clickUi(page, 'Play');
  await clickUi(page, 'Play solo');
  await waitRide(page);
  await expect(page.locator('dialog, aside, button:not(#boot-retry)')).toHaveCount(0);
});

test('Given a loaded shift, M opens a Bevy map above the HUD and returns to the same route', async ({page}) => {
  await page.goto('/?lat=52.4895&lon=13.3908');
  await waitRide(page);
  await page.keyboard.press('m');
  await expect.poll(async () => (await fact(page, 'paket-ui-regions'))?.panel).toBe('route');
  await clickUi(page, 'Map stop 2');
  await expect.poll(async () => (await fact(page, 'paket-hud'))?.selected).toBe(1);
  await page.screenshot({path:test.info().outputPath('bevy-route-map.png')});
  await page.keyboard.press('Escape');
  await expect.poll(async () => (await fact(page, 'paket-hud'))?.menu).toBe(false);
  expect((await fact(page, 'paket-hud')).selected).toBe(1);
  await page.screenshot({path:test.info().outputPath('bevy-shift.png')});
});

test('Given no streets at the chosen point, loading ends with a working neighbourhood choice', async ({page}) => {
  await page.route('**/api/osm?*', r => r.fulfill({json:{elements:[]}}));
  await page.goto('/?lat=51.23&lon=12.34');
  await expect.poll(async () => (await fact(page, 'paket-ride-loading')), {timeout:90000}).toBe('empty');
  await clickUi(page, 'Choose neighbourhood');
  await expect.poll(async () => (await fact(page, 'paket-ui-regions'))?.mode).toBe('map');
  await clickUi(page, 'Berlin');
});
