import {expect, test} from '@playwright/test';

test('a real shift loads the Quaternius pedestrians and traffic models', async ({page}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires the real WebGPU build.');
  const models = new Set<string>();
  const errors: string[] = [];
  page.on('response', response => {
    const path = new URL(response.url()).pathname;
    if (path.startsWith('/assets/street/') && response.ok()) models.add(path.split('/').pop()!);
  });
  page.on('console', message => {
    if (message.type() === 'error' && /assets\/street|gltf/i.test(message.text())) {
      errors.push(message.text());
    }
  });
  await page.goto('/?lat=52.4895&lon=13.3908');
  await page.getByRole('button', {name: 'Play', exact: true}).click();
  await expect(page.locator('#start')).toBeHidden({timeout: 30_000});
  await expect(page.locator('#route-direction')).toBeVisible({timeout: 45_000});
  await expect.poll(() => [...models].sort()).toEqual([
    'car-compact.glb', 'car-sedan.glb', 'car-suv.glb',
    'man-casual.glb', 'man-hoodie.glb', 'woman-casual.glb', 'woman-formal.glb',
  ]);
  expect(errors).toEqual([]);
  await page.locator('#game').screenshot({path: test.info().outputPath('street-models.png')});
});
