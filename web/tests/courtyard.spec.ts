import {observeGame,clickUi,fact,hud,selected,waitRide} from './bevy-ui';
import {expect, test} from '@playwright/test';

test.beforeEach(async({page})=>{await observeGame(page);});
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
  await waitRide(page);
  const stops=(await hud(page)).routes as {address:string}[];
  expect(stops.filter(stop=>stop.address==='Riemannstraße 21')).toHaveLength(1);
  expect(stops.filter(stop=>stop.address==='Riemannstraße 9')).toHaveLength(0);
});
