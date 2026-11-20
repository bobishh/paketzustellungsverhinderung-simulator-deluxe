import {expect,test} from '@playwright/test';
import {observeGame,clickUi,fillUi,fact,waitRide} from './bevy-ui';
test.beforeEach(async({page})=>{
 await observeGame(page);
 // Menu/HUD checks use real geometry without depending on public map services.
 await page.route('**/elevation/**',route=>route.fulfill({status:503}));
 await page.route('**/api/osm?*',route=>{
  const query=new URL(route.request().url()).searchParams;
  return query.get('x')==='0'&&query.get('y')==='0'&&!query.has('landscape')
   ? route.fulfill({path:new URL('../../fixtures/kreuzberg.json',import.meta.url).pathname,contentType:'application/json'})
   : route.fulfill({json:{elements:[]}});
 });
});
test('Bevy neighbourhood controls preserve arbitrary addresses and expose recoverable search failure',async({page})=>{
 test.setTimeout(120_000);
 await page.route('**/api/meshes',r=>r.fulfill({json:[]}));
 await page.route('**/api/address?*',r=>r.fulfill({json:[{name:'Dublin custom start',lat:53.3438,lon:-6.2546}]}));
 await page.goto('/');
 await clickUi(page,'Play');
 await clickUi(page,'Tokyo');
 await expect.poll(async()=>(await fact(page,'paket-lobby'))?.lat).toBeCloseTo(35.6616,4);
 await fillUi(page,'Street address or place','Dublin');await clickUi(page,'Find address or place');
 await clickUi(page,'Dublin custom start');
 await expect.poll(async()=>(await fact(page,'paket-lobby'))?.lon).toBeCloseTo(-6.2546,4);
 await expect(page.locator('button:not(#boot-retry),dialog,aside')).toHaveCount(0);
 await expect(page.locator('#bootstrap')).toBeHidden();
 await page.route('**/api/address?*',r=>r.fulfill({status:503}));await page.route('https://photon.komoot.io/**',r=>r.fulfill({status:503}));
 await fillUi(page,'Street address or place','Unavailable address');await clickUi(page,'Find address or place');
 await expect.poll(async()=>(await fact(page,'paket-ui-regions'))?.status).toContain('unavailable');
 await expect.poll(async()=>(await fact(page,'paket-ui-regions'))?.regions.some((r:any)=>r.label==='Start shift')).toBe(true);
 await page.screenshot({path:test.info().outputPath('bevy-map-and-controls.png')});
});
test('Bevy panels open in the real game and return to the same shift',async({page})=>{
 test.setTimeout(120_000);await page.goto('/?lat=52.4895&lon=13.3908');await waitRide(page);
 await clickUi(page,'Settings');
 await expect.poll(async()=>(await fact(page,'paket-hud'))?.menu).toBe(true);
 await clickUi(page,'Close · Esc');
 await expect.poll(async()=>(await fact(page,'paket-hud'))?.menu).toBe(false);
 await clickUi(page,'How to play');await clickUi(page,'Close · Esc');
 await page.screenshot({path:test.info().outputPath('bevy-shift-hud.png')});
});
test('technical bootstrap centres progress and leaves a retry after renderer failure',async({page})=>{
 await page.addInitScript(()=>{Object.defineProperty(navigator,'gpu',{value:{requestAdapter:async()=>null}});});
 await page.goto('/');
 await expect(page.locator('#bootstrap')).toBeVisible();
 await expect(page.locator('#boot-status')).toContainText('WebGPU is unavailable');
 await expect(page.locator('#boot-retry')).toBeVisible();
 const box=await page.locator('#bootstrap .spinner').boundingBox();
 expect(box).not.toBeNull();expect(Math.abs(box!.x+box!.width/2-page.viewportSize()!.width/2)).toBeLessThan(2);
});

for (const viewport of [{width:390,height:780},{width:844,height:390}]) {
 test(`Bevy lobby keeps map and pinned start controls reachable at ${viewport.width}x${viewport.height}`,async({page})=>{
  test.setTimeout(120_000);
  await page.setViewportSize(viewport);
  await page.route('**/api/meshes',r=>r.fulfill({json:[]}));
  await page.goto('/');await clickUi(page,'Play');
  const visible = async(label:string)=>{
   await expect.poll(async()=>{
    const region=(await fact(page,'paket-ui-regions'))?.regions.find((r:any)=>r.label===label);
    return Boolean(region && region.x>=-1 && region.y>=-1 && region.width>0 && region.height>0 && region.x+region.width<=viewport.width+1 && region.y+region.height<=viewport.height+1);
   }).toBe(true);
  };
  for(const label of ['Profile','Share mesh','How to play','Settings','My location','Street address or place','Make public','Start shift'])await visible(label);
  await page.screenshot({path:test.info().outputPath(`bevy-lobby-${viewport.width}.png`)});
  const start = (await fact(page,'paket-ui-regions')).regions.find((r:any)=>r.label==='Start shift');
  await page.mouse.move(viewport.width-145,viewport.height-170);
  await page.mouse.wheel(0,240);
  await visible('Invitation link');
  await visible('Start shift');
  expect((await fact(page,'paket-ui-regions')).regions.find((r:any)=>r.label==='Start shift')).toEqual(start);
  await page.goto('/?lat=52.4895&lon=13.3908');await waitRide(page);
  await expect.poll(async()=>{
   const view=await fact(page,'paket-ui-regions');
   const header=view?.regions.filter((r:any)=>['Profile','Share mesh','How to play','Settings'].includes(r.label));
   const headerBottom=header?.length ? Math.max(...header.map((r:any)=>r.y+r.height)) : Infinity;
   const map=view?.regions.find((r:any)=>r.label==='Map · M');
   const clock=view?.texts.find((r:any)=>r.text==='09:00'||r.text.startsWith('09:'));
   const delivery=view?.texts.find((r:any)=>r.text.startsWith('DELIVERY '));
   return Boolean(map&&clock&&delivery&&map.y>=headerBottom&&clock.y>=headerBottom&&(delivery.y>=map.y+map.height||delivery.x>=map.x+map.width));
  }).toBe(true);
 });
}
