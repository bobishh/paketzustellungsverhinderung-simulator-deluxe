import {expect, test} from '@playwright/test';

test('shows centered startup progress and lets the player retry a failed neighbourhood', async ({page}) => {
  await page.setViewportSize({width: 1000, height: 720});
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', {value: {requestAdapter: async () => ({})}});
  });
  let renderer: (() => void) | undefined;
  const waitForRenderer = new Promise<void>(resolve => {renderer = resolve;});
  await page.route('**/game/paket_game.js', async route => {
    await waitForRenderer;
    await route.fulfill({
      contentType: 'text/javascript',
      body: "export default () => dispatchEvent(new CustomEvent('paket-mode', {detail:'ride'}));",
    });
  });
  await page.goto('/');
  await page.locator('#play').click();
  const loading = page.locator('#world-loading');
  await expect(loading.getByRole('status')).toHaveText('Starting renderer…');
  const spinner = (await loading.locator('.spinner').boundingBox())!;
  expect(Math.abs(spinner.x + spinner.width / 2 - 500)).toBeLessThan(2);
  expect(Math.abs(spinner.y + spinner.height / 2 - 360)).toBeLessThan(2);
  renderer!();
  await expect(loading.getByRole('status')).toHaveText('Starting neighbourhood…');
  await page.evaluate(() => dispatchEvent(new CustomEvent('paket-ride-loading', {detail:'failed'})));
  await expect(loading.getByRole('status')).toHaveText('Neighbourhood could not load.');
  await expect(loading.getByRole('button', {name:'Choose neighbourhood'})).toBeVisible();
  await loading.getByRole('button', {name:'Retry map'}).click();
  await expect(loading.getByRole('status')).toHaveText('Retrying neighbourhood…');
  await page.evaluate(() => dispatchEvent(new CustomEvent('paket-hud', {detail:{ready:true}})));
  await expect(loading).toBeHidden();
});
