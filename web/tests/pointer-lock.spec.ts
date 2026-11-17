import {expect, test} from '@playwright/test';

if (process.env.PAKET_POINTER_LOCK) test.use({headless:false});

test('a rejected mouse capture stays visible and the next click can capture it', async ({page}) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', {value: {requestAdapter: async () => ({})}});
    let locked: Element|null = null;
    Object.defineProperty(document, 'pointerLockElement', {get: () => locked});
    let reject = true;
    Object.defineProperty(Element.prototype, 'requestPointerLock', {value: function (this: Element) {
      queueMicrotask(() => {
        if (reject) {
          reject = false;
          document.dispatchEvent(new Event('pointerlockerror'));
        } else {
          locked = this;
          document.dispatchEvent(new Event('pointerlockchange'));
        }
      });
    }});
  });
  await page.route('**/game/paket_game.js', route => route.fulfill({
    contentType: 'text/javascript',
    body: `export default async () => {
      dispatchEvent(new CustomEvent('paket-mode', {detail:'ride'}));
      dispatchEvent(new CustomEvent('paket-hud', {detail:{ready:true}}));
    };`,
  }));
  await page.goto('/');
  await page.getByRole('button', {name:'Play', exact:true}).click();
  const capture = page.getByRole('button', {name:'Capture mouse'});
  const locked = () => page.evaluate(() => document.pointerLockElement?.id ?? null);
  await capture.click({timeout:5000});
  await expect(page.locator('#notice')).toContainText('Mouse capture was blocked');
  expect(await locked()).toBeNull();
  await expect(capture).toBeEnabled();
  await capture.click();
  await expect.poll(locked).toBe('game');
  await expect(capture).toBeHidden();
});

test.describe('desktop pointer capture', () => {
  test.skip(!process.env.PAKET_REAL_GAME || !process.env.PAKET_POINTER_LOCK, 'Requires WebGPU and a desktop browser display.');

  test('the real game holds the mouse at the edge, releases for settings and captures again', async ({page}) => {
    await page.addInitScript(() => {
      Object.defineProperty(window, 'lookHeadings', {value: []});
      window.addEventListener('paket-radar', event => {
        const state = (event as CustomEvent<{heading:number}>).detail;
        (window as unknown as {lookHeadings:number[]}).lookHeadings.push(state.heading);
      });
    });
    await page.goto('/?lat=52.4895&lon=13.3908');
    await page.getByRole('button', {name:'Play', exact:true}).click();
    await expect(page.locator('#route-direction')).toBeVisible({timeout:45_000});
    await page.bringToFront();
    await page.keyboard.press('Shift');
    const canvas = page.locator('#game');
    const locked = () => page.evaluate(() => document.pointerLockElement?.id ?? null);
    await canvas.click({position:{x:620,y:320}});
    await expect.poll(locked).toBe('game');
    const heading = () => page.evaluate(() => (window as unknown as {lookHeadings:number[]}).lookHeadings.at(-1)!);
    const initial = await heading();
    await page.mouse.move(page.viewportSize()!.width + 30, 320);
    await expect.poll(async () => Math.abs(Math.sin((await heading()) - initial))).toBeGreaterThan(0.15);
    await expect.poll(locked).toBe('game');
    await page.keyboard.press('F1');
    await expect(page.locator('#settings-dialog')).toBeVisible();
    await expect.poll(locked).toBeNull();
    await page.getByRole('button', {name:/Back to work/}).click();
    await canvas.click({position:{x:620,y:320}});
    await expect.poll(locked).toBe('game');
    await page.evaluate(() => document.exitPointerLock());
    await expect.poll(locked).toBeNull();
    await expect(page.getByRole('button', {name:'Capture mouse'})).toBeVisible();
    await canvas.click({position:{x:620,y:320}});
    await expect.poll(locked).toBe('game');
  });
});
