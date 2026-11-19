import {expect,test,type Page} from '@playwright/test';
import {observeGame,clickUi,fillUi,fact} from './bevy-ui';

const ui=(page:Page)=>fact(page,'paket-ui-regions');
const text=async(page:Page)=>((await ui(page))?.texts??[]).map((value:{text:string})=>value.text).join('\n');
const region=async(page:Page,label:string)=>(await ui(page))?.regions.find((value:{label:string})=>value.label===label);
const bounds=(value:any)=>({x:value.x,y:value.y,width:value.width,height:value.height});

test.beforeEach(async({page})=>{
  await observeGame(page);
  await page.setViewportSize({width:1440,height:1200});
});

test('opening the real lobby loads the directory and keeps setup controls fixed through failure and refresh',async({page})=>{
  let release!:()=>void;
  const pending=new Promise<void>(resolve=>{release=resolve;});
  let requests=0;
  const invitation={protocol:'paket-web/3',origin:{lat:52.4895,lon:13.3908},room:'a'.repeat(64),host:'b'.repeat(64),relay:'https://relay.example.test/'};
  await page.route('**/api/meshes',async route=>{
    requests++;
    if(requests===1){await pending;await route.fulfill({status:503});}
    else if(requests===2)await route.fulfill({json:[]});
    else await route.fulfill({json:[invitation,{...invitation,room:'c'.repeat(64)}]});
  });
  await page.goto('/');await clickUi(page,'Play');await clickUi(page,'Friends');
  try {
    await expect.poll(()=>text(page)).toContain('Looking for public meshes');
    await expect.poll(async()=>Boolean(await region(page,'Refreshing…'))).toBe(true);
    const setup=['Invitation link','Join shift','Return to your mesh','Play solo','Play with friends'];
    const before=await Promise.all(setup.map(async label=>bounds(await region(page,label))));
    release();
    await expect.poll(()=>text(page)).toContain('Mesh list unavailable');
    expect(await Promise.all(setup.map(async label=>bounds(await region(page,label))))).toEqual(before);
    await clickUi(page,'Refresh');
    await expect.poll(()=>text(page)).toContain('No public meshes yet');
    expect(await Promise.all(setup.map(async label=>bounds(await region(page,label))))).toEqual(before);
    await clickUi(page,'Refresh');
    await expect.poll(()=>text(page)).toContain('2 public meshes');
    await expect.poll(async()=>(await ui(page))?.regions.filter((value:{label:string})=>value.label.includes('aaaaaa')||value.label.includes('cccccc')).length).toBe(2);
    expect(await Promise.all(setup.map(async label=>bounds(await region(page,label))))).toEqual(before);
    expect(requests).toBe(3);
    await clickUi(page,'Neighbourhood');
    await expect.poll(async()=>Boolean(await region(page,'My location'))).toBe(true);
    await expect.poll(async()=>Boolean(await region(page,'Street address or place'))).toBe(true);
    await expect.poll(async()=>Boolean(await region(page,'Find address or place'))).toBe(true);
    expect(await Promise.all(['Play solo','Play with friends'].map(async label=>bounds(await region(page,label))))).toEqual(before.slice(-2));
  } finally {release();}
});

test('a rejected invitation remains editable next to the real join error',async({page})=>{
  await page.route('**/api/meshes',route=>route.fulfill({json:[]}));
  await page.goto('/');await clickUi(page,'Play');await clickUi(page,'Friends');
  const invalid='https://example.test/#join=invalid';
  await fillUi(page,'Invitation link',invalid);
  await expect.poll(async()=>(await region(page,'Invitation link'))?.value).toBe(invalid);
  await clickUi(page,'Join shift');
  await expect.poll(()=>text(page)).toContain('Paste a valid Paket invite link.');
  expect((await region(page,'Invitation link')).value).toBe(invalid);
  expect((await ui(page))?.mode).toBe('map');
  expect((await region(page,'Join shift')).height).toBeGreaterThanOrEqual(36);
  await fillUi(page,'Invitation link','{}');await clickUi(page,'Join shift');
  await expect.poll(()=>text(page)).toContain('Paste a valid Paket invite link.');
  expect((await region(page,'Invitation link')).value).toBe('{}');
});

test('failed raster tiles retry in the real map while neighbourhood selection remains usable',async({page})=>{
  let release!:()=>void;
  const pending=new Promise<void>(resolve=>{release=resolve;});
  let fail=true,requests=0;
  await page.route('**/api/meshes',route=>route.fulfill({json:[]}));
  await page.route('https://tiles.openfreemap.org/styles/fiord', route => route.abort());
  await page.route('https://tile.openstreetmap.org/**/*.png',async route=>{
    requests++;
    if(fail){await pending;return route.fulfill({status:503});}
    return route.fulfill({path:new URL('../../assets/icon.png',import.meta.url).pathname,contentType:'image/png'});
  });
  await page.goto('/');await clickUi(page,'Play');
  try {
    await expect.poll(async()=>(await fact(page,'paket-map-loading'))?.pending).toBe(true);
    await expect.poll(async()=>(await region(page,'Play solo'))?.height??0).toBeGreaterThan(0);
    release();
    await expect.poll(async()=>(await fact(page,'paket-map-loading'))?.failed).toBe(true);
    await expect.poll(()=>text(page)).toContain('Some map tiles failed. Retry.');
    fail=false;
    const failedRequests=requests;
    await clickUi(page,'Retry map');
    await expect.poll(async()=>(await fact(page,'paket-map-loading'))?.ready).toBe(true);
    expect(requests).toBeGreaterThan(failedRequests);
    await clickUi(page,'Tokyo');
    await expect.poll(async()=>(await fact(page,'paket-lobby'))?.lat).toBeCloseTo(35.6616,4);
  } finally {release();}
});

// The neighbourhood loader is driven by actual tile failures, not synthetic HUD events.
test('a failed neighbourhood offers retry and starts the same shift after recovery',async({page})=>{
  let available=false;
  await page.route('**/elevation/**',route=>route.fulfill({status:503}));
  await page.route('**/api/osm?*',route=>{
    if(new URL(route.request().url()).searchParams.has('landscape'))return route.fulfill({json:{elements:[]}});
    return available?route.fulfill({json:{elements:[{type:'way',id:1,tags:{highway:'residential',name:'Recovered street'},geometry:[{lat:52.4895,lon:13.389},{lat:52.4895,lon:13.392}]}]}}):route.fulfill({status:503});
  });
  await page.goto('/?lat=52.4895&lon=13.3908');
  await expect.poll(()=>text(page),{timeout:90_000}).toContain('Neighbourhood could not load.');
  expect(await region(page,'Choose neighbourhood')).toBeDefined();
  available=true;await clickUi(page,'Retry map');
  await expect.poll(async()=>(await fact(page,'paket-hud'))?.ready,{timeout:90_000}).toBe(true);
  await expect.poll(async()=>Boolean(await region(page,'Choose neighbourhood'))).toBe(false);
});

test('the Bevy welcome artwork and Play remain inside desktop and phone screens',async({page})=>{
  await page.route('**/api/meshes',route=>route.fulfill({json:[]}));
  await page.goto('/');
  for(const size of [{width:1800,height:916},{width:1440,height:900},{width:844,height:390},{width:768,height:1024},{width:390,height:720},{width:360,height:600}]) {
    await page.setViewportSize(size);
    await expect.poll(async()=>{
      const view=await ui(page);
      const play=view?.regions.find((r:any)=>r.label==='Play');
      const art=view?.images.find((image:any)=>image.loaded&&image.path?.endsWith('landing/delivery-day.png'));
      const logo=view?.images.some((image:any)=>image.path?.endsWith('branding/drehlss-van.png'));
      return Boolean(play&&play.x>=0&&play.y>=0&&play.x+play.width<=size.width&&play.y+play.height<=size.height&&
        play.width<=220&&art?.width===size.width&&art?.height===size.height&&!logo&&
        !view.regions.some((r:any)=>['Profile','Share mesh','Fullscreen'].includes(r.label)));
    }).toBe(true);
    expect(await page.evaluate(()=>[document.documentElement.scrollWidth-innerWidth,document.documentElement.scrollHeight-innerHeight])).toEqual([0,0]);
    await page.screenshot({path:test.info().outputPath(`welcome-${size.width}x${size.height}.png`)});
  }
  await clickUi(page,'Settings');
  await expect.poll(async()=>(await ui(page))?.panel).toBe('settings');
  await clickUi(page,'Close · Esc');
  await clickUi(page,'Play');
  await expect.poll(async()=>Boolean(await region(page,'Play solo'))).toBe(true);
  await expect(page.locator('link[rel="icon"][type="image/svg+xml"]')).toHaveAttribute('href','/favicon.svg?v=cardboard-transparent');
  const icon=await page.request.get('/favicon.svg');expect(icon.ok()).toBe(true);
  expect(icon.headers()['content-type']).toContain('image/svg+xml');
});

test('guest play needs no Identity portal, while the real profile offers optional sign-in',async({page})=>{
  const portalRequests:string[]=[];
  page.on('request',request=>{if(new URL(request.url()).hostname==='identity.meta-uber-engineer.dev')portalRequests.push(request.url());});
  await page.route('**/api/meshes',route=>route.fulfill({json:[]}));
  await page.goto('/');await clickUi(page,'Play');
  expect(await region(page,'Sign in with Identity')).toBeUndefined();
  await clickUi(page,'Profile');
  await expect.poll(()=>text(page)).toContain('Your career');
  await expect.poll(async()=>Boolean(await region(page,'Sign in with Identity'))).toBe(true);
  await clickUi(page,'Close · Esc');
  await expect.poll(async()=>Boolean(await region(page,'Sign in with Identity'))).toBe(false);
  expect(portalRequests).toEqual([]);
});

test('the vector basemap keeps the Bevy map and start controls interactive', async ({page}) => {
  await page.route('https://tiles.openfreemap.org/styles/fiord', route => route.fulfill({json: {
    version: 8,
    sources: {district: {type: 'geojson', data: {
      type: 'Feature', properties: {}, geometry: {type: 'Polygon',
        coordinates: [[[13.2, 52.35], [13.6, 52.35], [13.6, 52.6], [13.2, 52.6], [13.2, 52.35]]]},
    }}},
    layers: [{id: 'land', type: 'fill', source: 'district', paint: {'fill-color': '#355d65'}}],
  }}));
  await page.route('**/api/meshes', route => route.fulfill({json: []}));
  await page.goto('/');
  await clickUi(page, 'Play');
  await expect(page.locator('#lobby-map[data-ready] canvas')).toBeVisible();
  const map = (await page.locator('#lobby-map').boundingBox())!;
  // Sample the composed browser frame to catch an opaque game canvas hiding the map.
  await expect.poll(async () => {
    const shot = await page.screenshot({clip: {
      x: Math.floor(map.x + map.width * 0.3), y: Math.floor(map.y + map.height * 0.3), width: 1, height: 1,
    }});
    return page.evaluate(async bytes => {
      const image = await createImageBitmap(new Blob([new Uint8Array(bytes)], {type: 'image/png'}));
      const canvas = new OffscreenCanvas(1, 1);
      const context = canvas.getContext('2d')!;
      context.drawImage(image, 0, 0);
      return Array.from(context.getImageData(0, 0, 1, 1).data);
    }, Array.from(shot));
  }).toEqual([53, 93, 101, 255]);
  const initial = await fact(page, 'paket-lobby');
  await page.mouse.click(map.x + map.width * 0.68, map.y + map.height * 0.55, {delay: 80});
  await expect.poll(async () => (await fact(page, 'paket-lobby'))?.lon).toBeGreaterThan(initial.lon + 0.0005);
  await expect.poll(async () => Boolean(await region(page, 'Play solo'))).toBe(true);
  await page.screenshot({path: test.info().outputPath('neighbourhood-picker.png')});
});
