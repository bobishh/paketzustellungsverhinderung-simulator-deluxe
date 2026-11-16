import {expect, test} from '@playwright/test';

test('the start screen shows the game and offers Play at desktop and phone widths', async ({page}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', {value: {requestAdapter: async () => null}});
  });
  await page.goto('/');

  const start = page.locator('#start');
  const title = start.getByRole('heading', {level: 1});
  const art = start.getByRole('img', {name: /courier.*dog/i});
  const play = start.getByRole('button', {name: 'Play', exact: true});
  await expect(art).toBeVisible();
  await expect.poll(() => art.evaluate((image: HTMLImageElement) => image.naturalWidth)).toBeGreaterThan(0);
  expect((await title.boundingBox())!.x + (await title.boundingBox())!.width)
    .toBeLessThanOrEqual((await art.boundingBox())!.x);

  await page.setViewportSize({width: 390, height: 720});
  await expect(play).toBeInViewport();
  await expect(art).toBeInViewport();
  expect(await start.evaluate(screen => screen.scrollWidth)).toBe(390);

  const position = await play.boundingBox();
  await play.click();
  await expect(start.getByRole('status')).toContainText('WebGPU is unavailable');
  await expect(play).toBeEnabled();
  expect(await play.boundingBox()).toEqual(position);
});
