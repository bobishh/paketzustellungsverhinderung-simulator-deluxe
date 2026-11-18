import {expect,test} from '@playwright/test';
import {observeGame,clickUi,fact,waitRide} from './bevy-ui';
if(process.env.PAKET_POINTER_LOCK)test.use({headless:false});
test.beforeEach(async({page})=>{await observeGame(page);});
test('a rejected capture stays available and the next trusted Bevy button click captures',async({page})=>{
 test.skip(!process.env.PAKET_REAL_GAME,'Requires the real game.');
 await page.addInitScript(()=>{
  let locked:Element|null=null,reject=true;
  Object.defineProperty(document,'pointerLockElement',{get:()=>locked});
  Object.defineProperty(Element.prototype,'requestPointerLock',{value:function(this:Element){queueMicrotask(()=>{if(reject){reject=false;document.dispatchEvent(new Event('pointerlockerror'));}else{locked=this;document.dispatchEvent(new Event('pointerlockchange'));}});}});
 });
 await page.goto('/?lat=52.4895&lon=13.3908');await waitRide(page);
 const locked=()=>page.evaluate(()=>document.pointerLockElement?.id??null);
 await clickUi(page,'Capture mouse');
 await expect.poll(()=>fact(page,'paket-notice')).toContain('Mouse capture was blocked');expect(await locked()).toBeNull();
 await clickUi(page,'Capture mouse');await expect.poll(locked).toBe('game');
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
    await waitRide(page);
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
    await expect.poll(async()=>(await fact(page,'paket-ui-regions'))?.panel).toBe('settings');
    await expect.poll(locked).toBeNull();
    await clickUi(page,'Close · Esc');
    await canvas.click({position:{x:620,y:320}});
    await expect.poll(locked).toBe('game');
    await page.evaluate(() => document.exitPointerLock());
    await expect.poll(locked).toBeNull();
    await expect.poll(async()=>(await fact(page,'paket-ui-regions'))?.regions.some((r:any)=>r.label==='Capture mouse')).toBe(true);
    await canvas.click({position:{x:620,y:320}});
    await expect.poll(locked).toBe('game');
  });
});
