import {expect, test} from '@playwright/test';

const scenarios = [
  {name:'feud', id:11, clue:'refuses parcels for', reply:'Say it is for the building manager'},
  {name:'full hallway', id:16, clue:'Their hall is full', reply:'Say you are collecting a return'},
  {name:'vacation', id:12, clue:'leaving in 30 seconds', reply:'Say it only takes a second'},
  {name:'sleeping baby', id:15, clue:'sleeping baby', reply:'Say the parcel is for them'},
  {name:'picky recipient', id:14, clue:'Set the box against', reply:'Say the parcel is for them'},
  {name:'dog return', id:10, clue:'A dog may take it back', reply:null},
] as const;

for (const scenario of scenarios) test(`a courier plays the ${scenario.name} dispatcher scenario`, async ({page}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires the real WebGPU game.');
  test.setTimeout(180_000);
  await page.setViewportSize({width: 640, height: 480});
  await page.addInitScript(() => {
    (globalThis as any).hudFrames = 0;
    addEventListener('paket-hud', () => (globalThis as any).hudFrames++);
    addEventListener('paket-radar', e => {
      (globalThis as any).radar = (e as CustomEvent).detail;
    });
  });
  const ll = (east: number, north: number) => ({
    lat: north / 6378137 * 180 / Math.PI,
    lon: east / 6378137 * 180 / Math.PI,
  });
  const fixture = {elements: [
    {type:'way', id:1, tags:{highway:'residential', name:'Test Street'}, geometry:[ll(-90,0),ll(90,0)]},
    ...[0, 12].map((x, i) => ({
      type:'way', id:[scenario.id,31][i], tags:{building:'yes','building:levels':'2','addr:street':'Test Street','addr:housenumber':String(i+1)},
      geometry:[ll(x-4,8),ll(x+4,8),ll(x+4,17),ll(x-4,17),ll(x-4,8)],
    })),
  ]};
  await page.route('**/elevation/**', r => r.fulfill({status:503}));
  await page.route('**/api/osm?*', r => r.fulfill({json:fixture}));
  await page.goto('/?lat=0&lon=0');
  await page.locator('#play').click();
  await expect(page.locator('#world-loading')).toBeHidden({timeout:90_000});
  const canvas = page.locator('#game');
  const key = (k: string) => page.keyboard.press(k, {delay:130});
  const walkToMarker = async (tolerance: number) => {
    await canvas.click({position:{x:320,y:225}});
    for (let i=0; i<45; i++) {
      const {position, target, heading} = await page.evaluate(() => (globalThis as any).radar);
      const dx = target[0]-position[0], dz = target[1]-position[1];
      if (Math.hypot(dx,dz)<tolerance) return;
      const forward = -Math.sin(heading)*dx-Math.cos(heading)*dz;
      const right = Math.cos(heading)*dx-Math.sin(heading)*dz;
      const moves = [];
      if (Math.abs(forward) > 0.8) moves.push(forward > 0 ? 'w' : 's');
      if (Math.abs(right) > 0.8) moves.push(right > 0 ? 'd' : 'a');
      for (const move of moves) await page.keyboard.down(move);
      await page.waitForTimeout(Math.hypot(dx,dz)>5 ? 500 : 220);
      for (const move of moves) await page.keyboard.up(move);
    }
    throw new Error('Could not reach the marked door');
  };
  await expect(page.locator('#stop-state')).toHaveText('In the van', {timeout:90_000});

  await canvas.focus();
  await key('e');
  const parcelState = page.locator('#stop-state');
  await expect(parcelState).toHaveText('In your hands');
  if (scenario.name === 'feud') await test.step('exchange and set aside an unrung parcel', async () => {
    await key('Tab');
    await expect(page.locator('#hud-action')).toContainText('Exchange for selected parcel');
    await key('e');
    await expect(parcelState).toHaveText('In your hands');
    await expect(page.locator('#notice')).toContainText('Parcel exchanged');
    await key('Shift+Tab');
    await expect(page.locator('#hud-action')).toContainText('Exchange for selected parcel');
    await key('e');
    await expect(parcelState).toHaveText('In your hands');
    await key('x');
    await expect(parcelState).toHaveText('Set aside');
    await key('Tab');
    await expect(parcelState).toHaveText('In the van');
    await key('Shift+Tab');
    await expect(page.locator('#hud-action')).toContainText('Pick up box');
    await key('e');
    await expect(parcelState).toHaveText('In your hands');
  });
  await walkToMarker(2.1);
  await key('r');
  const intercom = page.getByRole('group', {name:'Intercom'});
  await intercom.getByRole('button', {name:/recipient/i}).click();
  await expect(page.locator('#dispatcher-message')).toContainText(scenario.clue);
  await expect(page.locator('#dispatcher-timer')).toContainText('KPI');
  await intercom.getByRole('button', {name:'Close'}).click();
  if (scenario.name === 'picky recipient') {
    const parcel = page.locator('#stop-state');
    const pressUntilState = async (label: string) => {
      await page.keyboard.down('e');
      try {
        await expect(parcel).toHaveText(label, {timeout:12_000});
      } finally {
        await page.keyboard.up('e');
      }
      const frame = await page.evaluate(() => (globalThis as any).hudFrames);
      await page.waitForFunction(before => (globalThis as any).hudFrames > before, frame);
    };
    await pressUntilState('Photo needed');
    await pressUntilState('In your hands');
  }
  if (!scenario.reply) {
    await key('e');
    await expect(page.locator('#route-total')).toHaveText('1 / 2', {timeout:20_000});
    await expect(page.locator('#notice')).toContainText('returned the parcel to the real door · 0 KPI');
    return;
  }
  await walkToMarker(1.8);
  await key('r');
  await intercom.getByRole('button', {name:'Tell the truth'}).click();
  await expect(intercom).toContainText('refused');
  await intercom.getByRole('button', {name:scenario.reply}).click();
  await expect(intercom).toContainText('coming to the door');
  await intercom.getByRole('button', {name:'Close'}).click();
  await expect(page.locator('#hud-action')).toContainText('Hand over to the neighbor', {timeout:25_000});
  await key('e');
  await expect(page.locator('#route-total')).toHaveText('1 / 2');
  await expect(page.locator('#notice')).toContainText('Neighbor took the parcel');
  await expect.poll(async () => Number(await page.locator('#hud-score').textContent())).toBeGreaterThan(scenario.name === 'feud' ? 100 : 25);
});
