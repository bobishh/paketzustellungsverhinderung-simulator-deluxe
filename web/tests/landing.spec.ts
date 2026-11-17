import {expect, test} from '@playwright/test';

test('the start screen shows the game and offers Play at desktop and phone widths', async ({page}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', {value: {requestAdapter: async () => null}});
  });
  await page.goto('/');

  const start = page.locator('#start');
  const favicon = page.locator('link[rel="icon"][type="image/svg+xml"]');
  await expect(favicon).toHaveAttribute('href', '/favicon.svg?v=cardboard-transparent');
  const icon = await page.request.get('/favicon.svg');
  expect(icon.ok()).toBeTruthy();
  expect(icon.headers()['content-type']).toContain('image/svg+xml');
  const art = start.getByRole('img', {name: /courier.*dog/i});
  const play = start.getByRole('button', {name: 'Play', exact: true});
  await expect(art).toBeVisible();
  await expect.poll(() => art.evaluate((image: HTMLImageElement) => image.naturalWidth)).toBeGreaterThan(0);
  expect(await art.boundingBox()).toEqual(await start.boundingBox());

  for (const viewport of [{width: 1440, height: 900}, {width: 390, height: 720}, {width: 360, height: 600}]) {
    await page.setViewportSize(viewport);
    await expect(play).toBeInViewport();
    await expect(art).toBeInViewport();
    await expect(start.getByRole('link', {name: /source code/i})).toBeInViewport();
    expect(await start.evaluate(screen => [
      screen.scrollWidth - screen.clientWidth,
      screen.scrollHeight - screen.clientHeight,
      document.documentElement.scrollWidth - innerWidth,
      document.documentElement.scrollHeight - innerHeight,
    ])).toEqual([0, 0, 0, 0]);
  }

  const position = await play.boundingBox();
  await play.click();
  await expect(start.getByRole('status')).toContainText('WebGPU is unavailable');
  await expect(play).toBeEnabled();
  expect(await play.boundingBox()).toEqual(position);
});

test('guests can play and find the optional Identity link inside their profile', async ({page}) => {
  const portalRequests: string[] = [];
  page.on('request', request => {
    if (new URL(request.url()).hostname === 'identity.meta-uber-engineer.dev') {
      portalRequests.push(request.url());
    }
  });
  await page.setViewportSize({width: 360, height: 720});
  await page.goto('/');
  await expect(page.getByRole('button', {name: 'Play', exact: true})).toBeVisible();
  const signIn = page.getByRole('button', {name: 'Sign in with Identity'});
  await expect(signIn).toBeHidden();
  await expect.poll(() => page.locator('#profile-content').textContent()).toContain('Career KPI');
  await page.getByRole('button', {name: /Profile/}).click();
  await expect(signIn).toBeVisible();
  await expect(page.locator('#profile-dialog')).toContainText('Guest play starts without an account');
  await page.getByRole('button', {name: 'Close profile'}).click();
  await expect(signIn).toBeHidden();
  expect(portalRequests).toEqual([]);
});
