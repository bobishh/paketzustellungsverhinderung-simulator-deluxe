import {expect, test} from '@playwright/test';
import {clickUi, fact, observeGame, waitRide} from './bevy-ui';

test.beforeEach(async ({page}) => {
  await observeGame(page);
  await page.route('**/elevation/**', r => r.fulfill({status:503}));
  await page.route('**/api/meshes', r => r.fulfill({json:[]}));
  await page.route('**/api/osm?*', r => {
    const q = new URL(r.request().url()).searchParams;
    return q.get('x') === '0' && q.get('y') === '0' && !q.has('landscape')
      ? r.fulfill({path:new URL('../../fixtures/kreuzberg.json', import.meta.url).pathname, contentType:'application/json'})
      : r.fulfill({json:{elements:[]}});
  });
});

test('Given unavailable welcome artwork, when boot finishes, Play still opens the Bevy lobby', async ({page}) => {
  await page.route('**/assets/landing/**', r => r.fulfill({status:404}));
  await page.goto('/');
  await clickUi(page, 'Play');
  await clickUi(page, 'Start shift');
  await waitRide(page);
  await expect(page.locator('dialog, aside, button:not(#boot-retry)')).toHaveCount(0);
});

test('Given a loaded shift, M opens a Bevy map above the HUD and returns to the same route', async ({page}) => {
  await page.goto('/?lat=52.4895&lon=13.3908');
  await waitRide(page);
  await page.keyboard.press('m');
  await expect.poll(async () => (await fact(page, 'paket-ui-regions'))?.panel).toBe('route');
  await clickUi(page, 'Map stop 2');
  await expect.poll(async () => (await fact(page, 'paket-hud'))?.selected).toBe(1);
  await page.screenshot({path:test.info().outputPath('bevy-route-map.png')});
  await page.keyboard.press('Escape');
  await expect.poll(async () => (await fact(page, 'paket-hud'))?.menu).toBe(false);
  expect((await fact(page, 'paket-hud')).selected).toBe(1);
  await page.screenshot({path:test.info().outputPath('bevy-shift.png')});
});

test('Given no streets at the chosen point, loading ends with a working neighbourhood choice', async ({page}) => {
  await page.route('**/api/osm?*', r => r.fulfill({json:{elements:[]}}));
  await page.goto('/?lat=51.23&lon=12.34');
  await expect.poll(async () => (await fact(page, 'paket-ride-loading')), {timeout:90000}).toBe('empty');
  await clickUi(page, 'Choose neighbourhood');
  await expect.poll(async () => (await fact(page, 'paket-ui-regions'))?.mode).toBe('map');
  await clickUi(page, 'Berlin');
});


test('Given an active shift, the route and instructions stay visible and a ground arrow follows the selected objective', async ({page}) => {
  await page.goto('/?lat=52.4895&lon=13.3908');
  await waitRide(page);
  await expect.poll(async () => (await fact(page, 'paket-ui-regions'))?.regions.filter((r:any) => r.label?.startsWith('Route stop ')).length).toBeGreaterThan(1);
  const routes=(await fact(page, 'paket-hud')).routes;
  const target=routes.reduce((best:any,route:any)=>route.distance>best.distance?route:best);
  await clickUi(page, `Route stop ${target.index+1}`);
  await expect.poll(async () => (await fact(page, 'paket-hud'))?.selected).toBe(target.index);
  // Step back from the parked van so the marker has an unobstructed place on the road.
  const origin=(await fact(page,'paket-radar')).position;
  await page.keyboard.down('s');
  try {await expect.poll(async()=>{
    const at=(await fact(page,'paket-radar')).position;
    return Math.hypot(at[0]-origin[0],at[1]-origin[1]);
  }).toBeGreaterThan(3);} finally {await page.keyboard.up('s');}

  await expect.poll(async () => await fact(page, 'paket-navigation')).toMatchObject({visible:true});
  const marker=await fact(page,'paket-navigation');
  const dx=marker.target[0]-marker.position[0],dz=marker.target[2]-marker.position[2];
  expect((dx*marker.direction[0]+dz*marker.direction[2])/Math.hypot(dx,dz)).toBeGreaterThan(0.999);

  await expect.poll(async () => (await fact(page, 'paket-ui-regions'))?.texts.some((t:any) => t.text.includes('Get the parcel'))).toBe(true);
  await page.screenshot({path:test.info().outputPath('route-instructions-and-ground-arrow.png')});
  await clickUi(page, 'Settings');
  await expect.poll(async () => (await fact(page, 'paket-navigation'))?.visible).toBe(false);
  await clickUi(page, 'Close · Esc');
  await expect.poll(async () => await fact(page, 'paket-navigation')).toMatchObject({visible:true});
});


test('the compact lower-left HUD keeps the route and instruction together', async ({page}) => {
  await page.setViewportSize({width:1440,height:1000});
  await page.goto('/?lat=52.4895&lon=13.3908');
  await waitRide(page);
  await expect.poll(async () => {
    const ui = await fact(page, 'paket-ui-regions');
    const panel = ui?.panels?.find((p:any) => p.kind === 'route');
    const stops = ui?.regions.filter((r:any) => r.label?.startsWith('Route stop '));
    const instruction = ui?.texts.find((t:any) => t.text.includes('Get the parcel'));
    const radar = ui?.panels?.find((p:any) => p.kind === 'radar');
    return Boolean(panel && stops?.length > 1 && instruction && radar &&
      ui.panels.length === 2 && panel.x < 24 && panel.y > 490 &&
      panel.width <= 320 && panel.height <= 460 && panel.y + panel.height > 925 &&
      instruction.x >= panel.x && instruction.y >= panel.y &&
      instruction.x + instruction.width <= panel.x + panel.width + 1 &&
      instruction.y + instruction.height <= panel.y + panel.height + 1 && radar.width < 190);
  }).toBe(true);
  await page.screenshot({path:test.info().outputPath('compact-shift-hud.png')});
});

test('the navigation arrow sits directly above the driven van and points toward the selected stop', async ({page}) => {
  await page.goto('/?lat=52.4895&lon=13.3908');
  await waitRide(page);
  await page.keyboard.press('v');
  await expect.poll(async () => (await fact(page, 'paket-hud'))?.form).toBe('van');
  await expect.poll(async () => await fact(page, 'paket-navigation')).toMatchObject({visible:true});
  const arrow = await fact(page, 'paket-navigation');
  const van = await fact(page, 'paket-radar');
  expect(arrow.position[0]).toBeCloseTo(van.position[0], 2);
  expect(arrow.position[2]).toBeCloseTo(van.position[1], 2);
  expect(arrow.position[1] - van.altitude).toBeGreaterThan(1.2);
  const dx = arrow.target[0] - arrow.position[0], dz = arrow.target[2] - arrow.position[2];
  expect((dx * arrow.direction[0] + dz * arrow.direction[2]) / (Math.hypot(dx, dz) * Math.hypot(arrow.direction[0], arrow.direction[2]))).toBeGreaterThan(0.999);
  await page.screenshot({path:test.info().outputPath('arrow-over-van.png')});
});
