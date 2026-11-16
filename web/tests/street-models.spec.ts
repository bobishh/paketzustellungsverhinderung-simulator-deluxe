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
    'car-compact.glb', 'car-sedan.glb', 'car-sport.glb', 'car-suv.glb', 'car-taxi.glb',
    'courier.glb', 'delivery-van.glb', 'man-casual.glb', 'man-hoodie.glb',
    'shiba.glb', 'woman-casual.glb', 'woman-formal.glb',
  ]);
  expect(errors).toEqual([]);
  await page.locator('#game').screenshot({path: test.info().outputPath('street-models.png')});
});

test('the courier walks with both hands on the parcel and both soles clear of the road', async ({page}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires the real WebGPU build.');
  type Sample = {position: number[]; grip: number[]; soles: number[]; carrying: boolean};
  await page.addInitScript(() => {
    const state = window as unknown as Window & {courierSamples: Sample[]};
    state.courierSamples = [];
    window.addEventListener('paket-courier-pose', event => {
      state.courierSamples.push((event as CustomEvent<Sample>).detail);
    });
  });
  await page.goto('/?lat=52.4895&lon=13.3908&inspect-courier');
  await page.getByRole('button', {name: 'Play', exact: true}).click();
  await expect(page.locator('#hud-action')).toContainText('Collect this parcel', {timeout: 45_000});
  const samples = () => page.evaluate(() => (window as unknown as Window & {courierSamples: Sample[]}).courierSamples);
  await page.keyboard.press('e');
  await expect(page.locator('#stop-state')).toHaveText('In your hands');
  await expect.poll(async () => (await samples()).some(sample => sample.carrying)).toBeTruthy();

  await page.keyboard.down('w');
  try {
    await expect.poll(async () => {
      const carrying = (await samples()).filter(sample => sample.carrying);
      const start = carrying[0].position;
      const end = carrying.at(-1)!.position;
      return Math.hypot(end[0] - start[0], end[1] - start[1]);
    }, {timeout: 15_000}).toBeGreaterThan(8);
  } finally {
    await page.keyboard.up('w');
  }
  const carrying = (await samples()).filter(sample => sample.carrying).slice(-16);
  expect(carrying).toHaveLength(16);
  expect(Math.max(...carrying.flatMap(sample => sample.grip))).toBeLessThan(0.06);
  for (const foot of [0, 1]) {
    const clearance = carrying.map(sample => sample.soles[foot]);
    expect(Math.min(...clearance)).toBeGreaterThan(-0.02);
    expect(Math.min(...clearance)).toBeLessThan(0.025);
    expect(Math.max(...clearance)).toBeGreaterThan(0.035);
  }
  await test.info().attach('courier-pose', {body: JSON.stringify(carrying), contentType: 'application/json'});
  await page.locator('#game').screenshot({path: test.info().outputPath('courier-carry.png')});
});
