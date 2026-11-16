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
  const courier = await page.request.get('/assets/street/courier.glb');
  expect(courier.ok()).toBeTruthy();
  const glb = await courier.body();
  const model = JSON.parse(glb.toString('utf8', 20, 20 + glb.readUInt32LE(12)));
  expect(model.nodes.some((node: {name: string}) => node.name === 'CourierHead')).toBeTruthy();
  expect(model.nodes.some((node: {name: string}) => node.name === 'Worker_Head')).toBeFalsy();

  await page.locator('#game').screenshot({path: test.info().outputPath('street-models.png')});
  await page.keyboard.press('v');
  await expect(page.locator('#speedometer')).toBeVisible();
  await page.keyboard.down('w');
  try {
    await expect.poll(async () => Number(await page.locator('#speed-value').textContent())).toBeGreaterThan(8);
  } finally {
    await page.keyboard.up('w');
  }
});

test('the courier walks with both hands on the parcel and both soles clear of the road', async ({page}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires the real WebGPU build.');
  type Sample = {seconds: number; position: number[]; grip: number[]; soles: number[]; carrying: boolean};
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
  const moving = (await samples()).filter(sample => sample.carrying);
  const end = moving.at(-1)!.seconds;
  const stride = moving.filter(sample => sample.seconds > end - 2);
  const swings = stride.flatMap(({soles}) =>
    Math.max(...soles) > 0.045 && Math.abs(soles[0] - soles[1]) > 0.035
      ? [Number(soles[1] > soles[0])] : []);
  const steps = swings.filter((foot, i) => i > 0 && foot !== swings[i - 1]).length;
  expect(steps).toBeGreaterThanOrEqual(3);
  const carrying = moving.slice(-16);
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

test('scene quality keeps its resolution when a smaller canvas does not improve frame time', async ({page}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires the real WebGPU build.');
  await page.addInitScript(() => {
    let sample: typeof globalThis.paketFrame;
    Object.defineProperty(globalThis, 'paketFrame', {
      get: () => () => {},
      set: callback => { sample = callback; },
    });
    Object.assign(window, {
      sampleFrames: (ms: number, count: number) => {
        for (let i = 0; i < count; i++) sample(ms);
      },
    });
    localStorage.setItem('paket.render-quality.v2', JSON.stringify({pixels: innerWidth * (innerHeight - 66), tier: 7}));
  });
  await page.goto('/?lat=52.4895&lon=13.3908');
  await page.getByRole('button', {name: 'Play', exact: true}).click();
  await expect(page.locator('#world-loading')).toBeHidden({timeout: 45_000});
  await expect(page.locator('#route-direction')).toBeVisible({timeout: 45_000});
  const canvas = page.locator('#game');
  const initial = Number(await canvas.getAttribute('data-render-scale'));
  expect(initial).toBeGreaterThan(0.2);
  const sample = (ms: number, count: number) => page.evaluate(
    ([duration, frames]) => (window as unknown as {sampleFrames(ms: number, count: number): void}).sampleFrames(duration, frames),
    [ms, count],
  );

  await sample(40, 74);
  const trial = Number(await canvas.getAttribute('data-render-scale'));
  expect(trial).toBeLessThan(initial);
  await sample(40, 75);
  await expect(canvas).toHaveAttribute('data-render-scale', String(initial));
  await sample(40, 250);
  await expect(canvas).toHaveAttribute('data-render-scale', String(initial));
  await sample(52, 42);
  await expect(canvas).toHaveAttribute('data-render-scale', String(trial));
  await sample(21, 160);
  await expect(canvas).toHaveAttribute('data-render-scale', String(trial));
});

test('the delivery van has four full-size tires and clearance under its body', async ({request}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires the real game assets.');
  const response = await request.get('/assets/street/delivery-van.glb');
  expect(response.ok()).toBeTruthy();
  const glb = await response.body();
  type Bounds = {min: number[]; max: number[]};
  const asset = JSON.parse(glb.toString('utf8', 20, 20 + glb.readUInt32LE(12))) as {
    nodes: {name: string; mesh: number; translation?: number[]}[];
    meshes: {primitives: {attributes: {POSITION: number}}[]}[];
    accessors: Bounds[];
  };
  const bounds = (mesh: number) => asset.meshes[mesh].primitives.map(primitive => asset.accessors[primitive.attributes.POSITION]);
  const body = asset.nodes.find(node => node.name === 'Body')!;
  expect(Math.min(...bounds(body.mesh).map(part => part.min[1]))).toBeGreaterThan(0.24);
  const wheels = asset.nodes.filter(node => node.name.startsWith('Wheel'));
  expect(wheels).toHaveLength(4);
  for (const wheel of wheels) {
    const [tire] = bounds(wheel.mesh);
    const center = wheel.translation!;
    expect(center[1] + tire.min[1]).toBeCloseTo(0, 4);
    expect(tire.max[1] - tire.min[1]).toBeGreaterThan(0.55);
    expect(Math.abs(center[0]) + tire.max[0]).toBeGreaterThan(0.88);
  }
});
