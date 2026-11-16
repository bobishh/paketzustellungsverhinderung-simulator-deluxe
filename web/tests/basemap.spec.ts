import {expect, test} from '@playwright/test';

test.use({launchOptions: {args: ['--enable-unsafe-swiftshader']}});

test.beforeEach(async ({page}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', {value: {requestAdapter: async () => ({})}});
  });
  await page.route('**/api/meshes', route => route.fulfill({json: []}));
  await page.route('**/game/paket_game.js', route => route.fulfill({
    contentType: 'text/javascript',
    body: `export default async () => {
      dispatchEvent(new CustomEvent('paket-mode', {detail: 'map'}));
      function publish(lat, lon, label) {
        dispatchEvent(new CustomEvent('paket-lobby', {
          detail: {lat, lon, label, zoom: 16, pending: false, ready: false}
        }));
      }
      publish(52.4895, 13.3908, 'Berlin · Kreuzberg');
      document.querySelector('[data-city^="Tokyo"]').addEventListener('click', () =>
        publish(35.6616, 139.6682, 'Tokyo · Shimokitazawa'));
    }`,
  }));
});

test('the styled map appears and recenters on the selected city', async ({page}) => {
  await page.route('https://tiles.openfreemap.org/styles/dark', route => route.fulfill({
    json: {
      version: 8,
      sources: {
        tokyo: {
          type: 'geojson',
          data: {
            type: 'Feature',
            properties: {},
            geometry: {
              type: 'Polygon',
              coordinates: [[[139.5, 35.5], [139.9, 35.5], [139.9, 35.8], [139.5, 35.8], [139.5, 35.5]]],
            },
          },
        },
      },
      layers: [
        {id: 'background', type: 'background', paint: {'background-color': '#223344'}},
        {id: 'tokyo', source: 'tokyo', type: 'fill', paint: {'fill-color': '#9b5632'}},
      ],
    },
  }));
  await page.goto('/');
  await page.getByRole('button', {name: 'Play', exact: true}).click();
  const canvas = page.locator('#lobby-map canvas');
  await expect(canvas).toBeVisible();
  await expect(page.locator('#world-loading')).toBeHidden();
  await expect(page.locator('#lobby-map-credit')).toBeVisible();
  await page.getByRole('button', {name: 'Tokyo', exact: true}).click();
  await expect(page.locator('#selected-place')).toHaveText('Tokyo · Shimokitazawa');
  expect(await page.evaluate(() => globalThis.paketCommands)).toContainEqual({
    action: 'select', lat: 35.6616, lon: 139.6682,
  });
  await expect.poll(async () => {
    const screenshot = await canvas.screenshot();
    return page.evaluate(async png => {
      const image = new Image();
      image.src = 'data:image/png;base64,' + png;
      await image.decode();
      const sample = document.createElement('canvas').getContext('2d')!;
      sample.drawImage(image, 0, 0);
      return Array.from(sample.getImageData(40, 120, 1, 1).data).slice(0, 3);
    }, screenshot.toString('base64'));
  }).toEqual([155, 86, 50]);
  await page.evaluate(() => dispatchEvent(new CustomEvent('paket-mode', {detail: 'ride'})));
  await expect(page.locator('#lobby-map canvas')).toHaveCount(0);
});

test('a failed vector style asks the game to load the raster map', async ({page}) => {
  await page.route('https://tiles.openfreemap.org/styles/dark', route => route.fulfill({status: 503}));
  await page.goto('/');
  await page.getByRole('button', {name: 'Play', exact: true}).click();
  await expect.poll(() => page.evaluate(() => globalThis.paketCommands)).toContainEqual({
    action: 'raster-map',
  });
  await expect(page.locator('#lobby-map canvas')).toHaveCount(0);
  await expect(page.locator('#world-loading')).toBeVisible();
  await expect(page.getByRole('button', {name: 'Play solo'})).toBeEnabled();
});
