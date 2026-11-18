import {observeGame,clickUi,fact,hud,selected,waitRide} from './bevy-ui';
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
  await observeGame(page);
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
  await waitRide(page);
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
  await expect.poll(async()=>(await selected(page))?.state,{timeout:90_000}).toBe('in-van');

  const stops=(await hud(page)).routes as {name:string}[];expect(stops).toHaveLength(2);
  const names=stops.map(stop=>stop.name);expect(new Set(names.map(name=>name.split(' ').at(-1))).size).toBe(2);
  await clickUi(page,'Deliveries');await clickUi(page,/Test Street 1/);await clickUi(page,'Close · Esc');
  expect((await selected(page)).address).toBe('Test Street 1');
  await canvas.focus();
  await key('e');
  const parcelState=async()=>(await selected(page))?.state;
  await expect.poll(parcelState).toBe('carrying');
  if (scenario.name === 'feud') await test.step('exchange and set aside an unrung parcel', async () => {
    await key('Tab');
    await expect.poll(async()=>(await hud(page)).prompt).toContain('Exchange for selected parcel');
    await key('e');
    await expect.poll(parcelState).toBe('carrying');
    await expect.poll(async()=>(await hud(page)).status).toContain('Parcel exchanged');
    await key('Shift+Tab');
    await expect.poll(async()=>(await hud(page)).prompt).toContain('Exchange for selected parcel');
    await key('e');
    await expect.poll(parcelState).toBe('carrying');
    await key('x');
    await expect.poll(parcelState).toBe('set-aside');
    await key('Tab');
    await expect.poll(parcelState).toBe('in-van');
    await key('Shift+Tab');
    await expect.poll(async()=>(await hud(page)).prompt).toContain('Pick up box');
    await key('e');
    await expect.poll(parcelState).toBe('carrying');
  });
  await walkToMarker(2.1);
  const recipient=(await selected(page)).name;
  const intercom=async()=>(await hud(page)).intercom;
  if (scenario.name === 'feud') {
    await key('x');
    await expect.poll(parcelState).toBe('set-aside');
    await key('F1');
    await expect.poll(async()=>(await fact(page,'paket-ui-regions'))?.panel).toBe('settings');
    await key('r');
    expect(await intercom()).toBeNull();
    await clickUi(page,'Close · Esc');
    await canvas.focus();
  }
  await key('r');
  await expect.poll(intercom,{timeout:5_000}).toBeTruthy();
  if (scenario.name === 'feud') {
    await clickUi(page,'Close · Esc');
    await key('e');
    await expect.poll(parcelState).toBe('carrying');
    await key('r');
  }
  await clickUi(page,`Ring recipient · ${recipient}`);
  await expect.poll(async()=>(await hud(page)).dispatcher?.message).toContain(scenario.clue);
  await expect.poll(async()=>(await hud(page)).dispatcher?.timer).toContain('KPI');
  await clickUi(page,'Close · Esc');
  if (scenario.name === 'picky recipient') {
    const parcel=async()=>(await selected(page))?.state;
    const pressUntilState = async (label: string) => {
      await page.keyboard.down('e');
      try {
        await expect.poll(parcel,{timeout:12_000}).toBe(({ 'Photo needed':'photo-due','In your hands':'carrying'} as Record<string,string>)[label]);
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
    await expect.poll(async()=>(await hud(page)).delivered,{timeout:20_000}).toBe(1);
    await expect.poll(async()=>(await hud(page)).status).toContain('returned the parcel to the real door · 0 KPI');
    return;
  }
  await walkToMarker(1.8);
  await key('r');
  await clickUi(page,'Tell the truth');
  await expect.poll(async()=>JSON.stringify(await intercom())).toContain('refused');
  await clickUi(page,scenario.reply);
  await expect.poll(async()=>JSON.stringify(await intercom())).toContain('coming to the door');
  await clickUi(page,'Close · Esc');
  await expect.poll(async()=>(await hud(page)).prompt,{timeout:25_000}).toContain('Hand over to the neighbor');
  await key('e');
  await expect.poll(async()=>(await hud(page)).delivered).toBe(1);
  await expect.poll(async()=>(await hud(page)).status).toContain('Neighbor took the parcel');
  await expect.poll(async () => Number((await hud(page)).score)).toBeGreaterThan(scenario.name === 'feud' ? 100 : 25);
});
