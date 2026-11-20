import {observeGame,clickUi,fact,hud,waitRide} from './bevy-ui';
import {test,expect,type Page} from '@playwright/test';

test.beforeEach(async({page})=>{await observeGame(page);});
async function muteInSettings(page: Page) {
  await clickUi(page, 'Settings');
  let control: {x: number; y: number; width: number; height: number} | undefined;
  await expect.poll(async () => {
    control = (await fact(page, 'paket-ui-regions'))?.regions.find((region: {label: string; y: number}) =>
      region.label === 'Sound: on' && region.y > 80);
    return Boolean(control);
  }).toBe(true);
  const {x, y, width, height} = control!;
  await page.mouse.click(x + width / 2, y + height / 2, {delay: 60});
  await expect.poll(() => page.evaluate(() => localStorage.getItem('paket.sound'))).toBe('off');
}

test('a real shift plays recorded footsteps and a throttle-driven engine, and settings silence it',async({page})=>{
 test.skip(!process.env.PAKET_REAL_GAME,'Requires the real WebGPU game.');
 test.setTimeout(120_000);
 await page.addInitScript(()=>{
  const state=globalThis as any;state.audioStarts=[];state.audioRates=[];state.streetAudio=[];state.audioStops=0;
  const create=AudioContext.prototype.createBufferSource;
  AudioContext.prototype.createBufferSource=function(){
   const source=create.call(this),start=source.start.bind(source),stop=source.stop.bind(source);
   source.start=(...args)=>{state.audioStarts.push({loop:source.loop,offset:args[1],duration:args[2]});start(...args);};
   source.stop=(...args)=>{state.audioStops++;stop(...args);};
   const target=source.playbackRate.setTargetAtTime.bind(source.playbackRate);
   source.playbackRate.setTargetAtTime=(value,...args)=>{state.audioRates.push(value);return target(value,...args);};
   return source;
  };
  addEventListener('street-audio',event=>state.streetAudio.push((event as CustomEvent).detail));
 });
 await page.route('**/elevation/**',route=>route.fulfill({status:503}));
 const point=(east:number,north:number)=>({lat:52.4895+north/111319.49,lon:13.3908+east/67783});
 await page.route('**/api/osm?*',route=>route.fulfill({json:{elements:[
  {type:'way',id:721,tags:{highway:'residential',name:'Audio Street'},geometry:[point(-110,0),point(125,0)]},
  {type:'way',id:722,tags:{building:'yes','addr:housenumber':'1','addr:street':'Audio Street'},geometry:[point(-8,12),point(8,12),point(8,25),point(-8,25),point(-8,12)]},
 ]}}));
 await page.goto('/?lat=52.4895&lon=13.3908');
 await waitRide(page);
 await expect.poll(()=>page.evaluate(()=>(globalThis as any).streetAudio.length),{timeout:45_000}).toBeGreaterThan(0);
 // A direct ride URL has no preceding Play click. Unlock audio with real input.
 await page.locator('#game').focus();
 await page.keyboard.press('Shift',{delay:80});
 await expect.poll(()=>page.evaluate(()=>(globalThis as any).audioStarts.filter((source:any)=>source.loop).length)).toBeGreaterThan(0);
 await page.locator('#game').focus();
 await page.keyboard.press('w',{delay:500});
 await expect.poll(()=>page.evaluate(()=>(globalThis as any).audioStarts.filter((source:any)=>!source.loop).length)).toBeGreaterThan(0);
 await page.keyboard.press('v');
 await expect.poll(async()=>(await hud(page)).form).toBe('van');
 await page.keyboard.down('w');
 try{
  await expect.poll(()=>page.evaluate(()=>(globalThis as any).streetAudio.some((state:any)=>state.form==='van'&&state.throttle===1&&state.speed>5))).toBeTruthy();
  await expect.poll(()=>page.evaluate(()=>(globalThis as any).audioRates.some((value:number)=>value>1.05))).toBeTruthy();
 }finally{await page.keyboard.up('w');}
 await muteInSettings(page);
 const muted=await page.evaluate(()=>(globalThis as any).audioStarts.length);
 await clickUi(page,'Close · Esc');
 await page.keyboard.press('w',{delay:600});
 expect(await page.evaluate(()=>(globalThis as any).audioStarts.length)).toBe(muted);
 expect(await page.evaluate(()=>localStorage.getItem('paket.sound'))).toBe('off');
 expect(await page.evaluate(()=>(globalThis as any).audioStops)).toBeGreaterThan(0);
 await page.screenshot({path:test.info().outputPath('recorded-street-audio.png')});
});

test('a running dog plays recorded paw taps and stays quiet at rest', async ({page}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires the real WebGPU game.');
  await page.addInitScript(() => {
    const starts: {loop: boolean; bufferLength: number}[] = [];
    Reflect.set(window, 'pawAudioStarts', starts);
    const createBufferSource = AudioContext.prototype.createBufferSource;
    AudioContext.prototype.createBufferSource = function () {
      const source = createBufferSource.call(this);
      const start = source.start.bind(source);
      source.start = (...args) => {
        starts.push({loop: source.loop, bufferLength: source.buffer?.duration ?? 0});
        start(...args);
      };
      return source;
    };
  });
  await page.route('**/elevation/**', route => route.fulfill({status: 503}));
  await page.route('**/api/osm?*', route => {
    const query = new URL(route.request().url()).searchParams;
    return query.get('x') === '0' && query.get('y') === '0' && !query.has('landscape')
      ? route.fulfill({path: new URL('../../fixtures/kreuzberg.json', import.meta.url).pathname, contentType: 'application/json'})
      : route.fulfill({json: {elements: []}});
  });
  await page.goto('/');
  await clickUi(page, 'Play');
  await clickUi(page, 'Dog');
  await clickUi(page, 'Start shift');
  await expect.poll(async () => (await hud(page)).form, {timeout: 25_000}).toBe('dog');
  const effects = () => page.evaluate(() => (Reflect.get(window, 'pawAudioStarts') as {
    loop: boolean; bufferLength: number;
  }[]).filter(start => !start.loop));
  expect(await effects()).toEqual([]);
  await page.keyboard.down('Shift');
  try {
    await page.keyboard.press('w', {delay: 500});
    await page.keyboard.press('s', {delay: 700});
  } finally {
    await page.keyboard.up('Shift');
  }
  await expect.poll(async () => (await effects()).length).toBeGreaterThanOrEqual(3);
  await expect.poll(async () => new Set((await effects()).map(effect => effect.bufferLength)).size).toBeGreaterThan(1);
  for (const effect of await effects()) expect(effect.bufferLength).toBeLessThan(0.2);
  await muteInSettings(page);
  const muted = (await effects()).length;
  await clickUi(page, 'Close · Esc');
  await page.keyboard.press('w', {delay: 650});
  expect(await effects()).toHaveLength(muted);
});
