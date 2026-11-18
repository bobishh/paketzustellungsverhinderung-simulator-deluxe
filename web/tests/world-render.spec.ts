import {expect, test} from '@playwright/test';
import {fact, observeGame, waitRide} from './bevy-ui';

test.use({viewport: {width: 2053, height: 1269}, deviceScaleFactor: 2,
  launchOptions: {args: ['--enable-unsafe-webgpu', '--force-device-scale-factor=2',
    ...(process.platform === 'linux' && process.env.CI === 'true'
      ? ['--enable-unsafe-swiftshader', '--use-angle=swiftshader', '--use-vulkan=swiftshader', '--enable-features=Vulkan'] : [])]}});
test.setTimeout(90_000);

test('the browser composites the street and updates its pixels when the courier moves', async ({page}) => {
  await observeGame(page);
  await page.route('**/elevation/**', route => route.fulfill({
    path: new URL('../../fixtures/elevation-slope.png', import.meta.url).pathname, contentType: 'image/png',
  }));
  await page.route('**/api/osm?*', route => {
    const query = new URL(route.request().url()).searchParams;
    return query.get('x') === '0' && query.get('y') === '0' && !query.has('landscape')
      ? route.fulfill({path: new URL('../../fixtures/kreuzberg.json', import.meta.url).pathname, contentType: 'application/json'})
      : route.fulfill({json: {elements: []}});
  });
  await page.goto('/?lat=52.4895&lon=13.3908');
  await waitRide(page);
  const before = (await fact(page, 'paket-radar')).position;
  const clip = {x: 500, y: 600, width: 900, height: 300};
  const image = await page.screenshot({clip});
  await page.keyboard.down('w');
  try {
    await expect.poll(async () => {
      const position = (await fact(page, 'paket-radar')).position;
      return Math.hypot(position[0] - before[0], position[1] - before[1]);
    }).toBeGreaterThan(0.5);
  } finally {await page.keyboard.up('w');}
  expect((await page.screenshot({clip})).equals(image), 'the street must render new pixels after movement').toBe(false);
  await page.screenshot({path: test.info().outputPath('moving-courier-street.png')});
});
