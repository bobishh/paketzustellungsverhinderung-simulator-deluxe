import {test,expect} from '@playwright/test';

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
 await page.getByRole('button',{name:'Play',exact:true}).click();
 await expect(page.locator('#world-loading')).toBeHidden({timeout:45_000});
 await expect.poll(()=>page.evaluate(()=>(globalThis as any).streetAudio.length),{timeout:45_000}).toBeGreaterThan(0);
 await expect.poll(()=>page.evaluate(()=>(globalThis as any).audioStarts.filter((source:any)=>source.loop).length)).toBeGreaterThan(0);
 await page.locator('#game').focus();
 await page.keyboard.press('w',{delay:500});
 await expect.poll(()=>page.evaluate(()=>(globalThis as any).audioStarts.filter((source:any)=>!source.loop).length)).toBeGreaterThan(0);
 await page.keyboard.press('v');
 await expect(page.locator('#speedometer')).toBeVisible();
 await page.keyboard.down('w');
 try{
  await expect.poll(()=>page.evaluate(()=>(globalThis as any).streetAudio.some((state:any)=>state.form==='van'&&state.throttle===1&&state.speed>5))).toBeTruthy();
  await expect.poll(()=>page.evaluate(()=>(globalThis as any).audioRates.some((value:number)=>value>1.05))).toBeTruthy();
 }finally{await page.keyboard.up('w');}
 await page.locator('#settings').click();
 await page.locator('#game-sound').uncheck();
 const muted=await page.evaluate(()=>(globalThis as any).audioStarts.length);
 await page.locator('#resume-game').click();
 await page.keyboard.press('w',{delay:600});
 expect(await page.evaluate(()=>(globalThis as any).audioStarts.length)).toBe(muted);
 expect(await page.evaluate(()=>localStorage.getItem('paket.sound'))).toBe('off');
 expect(await page.evaluate(()=>(globalThis as any).audioStops)).toBeGreaterThan(0);
 await page.screenshot({path:test.info().outputPath('recorded-street-audio.png')});
});
