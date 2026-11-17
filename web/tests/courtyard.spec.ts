import {expect, test} from '@playwright/test';

test('the route skips the closed courtyard and keeps the street address', async ({page}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires the real WebGPU game.');
  test.setTimeout(120_000);
  const fixture = new URL('../../fixtures/riemannstrasse-courtyard.json', import.meta.url).pathname;
  await page.route('**/elevation/**', route => route.fulfill({status:503}));
  await page.route('**/api/osm?*', route => route.fulfill(
    new URL(route.request().url()).searchParams.has('landscape')
      ? {json:{elements:[]}}
      : {path:fixture, contentType:'application/json'},
  ));
  await page.goto('/?lat=52.4895&lon=13.3908');
  await page.locator('#play').click();
  await expect(page.locator('#stop-state')).toHaveText('In the van', {timeout:90_000});
  await page.locator('#route-toggle').click();
  const stops = page.locator('#route-list .route-copy b');
  await expect(stops.filter({hasText:/^Riemannstraße 21$/})).toHaveCount(1);
  await expect(stops.filter({hasText:/^Riemannstraße 9$/})).toHaveCount(0);
});
