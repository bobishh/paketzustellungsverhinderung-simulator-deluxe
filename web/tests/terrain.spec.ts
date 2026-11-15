import {expect,test} from '@playwright/test';

test('the real game walks uphill, shows the stadium and joins both sidewalks to a road bridge', async ({page}) => {
  test.skip(!process.env.PAKET_REAL_GAME, 'Requires WebGPU.');
  test.setTimeout(150_000);
  await page.setViewportSize({width:640,height:480});
  await page.addInitScript(()=>{
    const w = globalThis as any;
    for (const [name,key] of [['paket-radar','terrainRadar'],['paket-map-roads','terrainRoads'],['paket-map-sports','terrainSports']]) {
      addEventListener(name, e => w[key] = (e as CustomEvent).detail);
    }
  });
  const lat=52.26793, lon=10.53202;
  const ll=(x:number,n:number)=>({lat:lat+n/6378137*180/Math.PI,lon:lon+x/6378137*180/Math.PI/Math.cos(lat*Math.PI/180)});
  const line=(id:number,tags:Record<string,string>,points:number[][])=>({type:'way',id,tags,geometry:points.map(([x,y])=>ll(x,y))});
  const box=(id:number,tags:Record<string,string>,x:number,y:number,w:number,h:number)=>line(id,tags,[[x,y],[x+w,y],[x+w,y+h],[x,y+h],[x,y]]);
  const ways={elements:[
    line(1,{highway:'residential',name:'Canal Road',width:'8',sidewalk:'separate'},[[-100,0],[120,0]]),
    ...[-7,7].map((y,i)=>line(2+i,{highway:'path',footway:'sidewalk',bridge:'yes',layer:'1'},[[20,y],[66,y]])),
    box(4,{leisure:'stadium'},-62,17,65,60),
    box(5,{leisure:'pitch',sport:'soccer'},-55,25,49,43),
    box(6,{building:'yes','addr:housenumber':'6','addr:street':'Canal Road'},5,12,7,8),
  ]};
  const water={elements:[box(7,{natural:'water'},29,-40,26,80)]};
  await page.route('**/elevation/**/*.png', r=>r.fulfill({path:new URL('../../fixtures/elevation-slope.png',import.meta.url).pathname,contentType:'image/png'}));
  await page.route('**/api/osm?*',r=>r.fulfill({json:new URL(r.request().url()).searchParams.has('landscape')?water:ways}));
  await page.goto(`/?lat=${lat}&lon=${lon}`);
  await page.locator('#play').click();
  await expect(page.locator('#terrain-credit')).toBeVisible({timeout:90_000});
  await expect.poll(()=>page.evaluate(()=>(globalThis as any).terrainSports?.length)).toBe(2);
  const roads = await page.evaluate(()=>(globalThis as any).terrainRoads as {bridge:boolean; motorRoad:boolean; width:number; points:number[][]}[]);
  const bridge = roads.filter(road=>road.bridge);
  expect(bridge).toHaveLength(1);
  expect(bridge[0].motorRoad).toBe(true);
  expect(bridge[0].width).toBeGreaterThan(14);
  const radar=()=>page.evaluate(()=>(globalThis as any).terrainRadar as {position:number[];altitude:number;heading:number});
  const start=await radar();
  const advance = Math.sin(start.heading) < 0 ? 'w' : 's';
  await page.locator('#game').click({position:{x:320,y:230}});
  await page.keyboard.down(advance);
  try {
    await expect.poll(async()=>(await radar()).position[0]-start.position[0],{timeout:35_000}).toBeGreaterThan(11);
  } finally {
    await page.keyboard.up(advance);
  }
  expect((await radar()).altitude-start.altitude).toBeGreaterThan(0.65);
});
