import {test,expect} from '@playwright/test';
test('Given delayed map and mesh requests, loading stays visible; peers and target get distinct ring markers',async({page})=>{
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(()=>Object.defineProperty(navigator,'gpu',{value:{requestAdapter:async()=>({})}}));
  await page.route('**/game/paket_game.js',r=>r.fulfill({contentType:'text/javascript',body:`export default async()=>dispatchEvent(new CustomEvent('paket-mode',{detail:'map'}))`}));
  let release!:()=>void;const pending=new Promise<void>(r=>release=r);
  let requests=0;
  await page.route('**/api/meshes',async r=>{if(requests++===0){await pending;await r.fulfill({status:503,body:'Unavailable'});}else await r.fulfill({contentType:'application/json',body:'[]'});});
  await page.goto('/');await page.locator('#play').click();
  await expect(page.locator('#mesh-list-spinner')).toBeVisible();
  await expect(page.locator('#world-loading')).toBeVisible();
  const before=await page.locator('#refresh-meshes').boundingBox();release();
  await expect(page.locator('#mesh-list-spinner')).toBeHidden();
  await expect(page.locator('#mesh-list-status')).toContainText('unavailable');
  expect(await page.locator('#refresh-meshes').boundingBox()).toEqual(before);
  await page.locator('#refresh-meshes').click();
  await expect(page.locator('#mesh-list-status')).toContainText('No public meshes yet');
  await page.evaluate(()=>dispatchEvent(new CustomEvent('paket-map-loading',{detail:{pending:false,ready:false,failed:true}})));
  await expect(page.locator('#retry-map')).toBeVisible();
  await page.locator('#retry-map').click();
  await expect(page.locator('#world-loading .spinner')).toBeVisible();
  expect(await page.evaluate(()=>(globalThis as any).paketCommands.some((c:any)=>c.action==='retry-map'))).toBe(true);
  await page.evaluate(()=>{
    dispatchEvent(new CustomEvent('paket-map-loading',{detail:{pending:false,ready:true,failed:false}}));
    dispatchEvent(new CustomEvent('paket-mode',{detail:'ride'}));
    dispatchEvent(new CustomEvent('paket-hud',{detail:{ready:true,routes:[],selected:0,interaction:false,minute:540,day:1,form:'courier',navigation:{turn:'left',distance:24}}}));
    dispatchEvent(new CustomEvent('paket-members',{detail:[{id:'a'.repeat(64),name:'Alice',slot:0},{id:'b'.repeat(64),name:'Bob',slot:1}]}));
    dispatchEvent(new CustomEvent('paket-radar',{detail:{position:[0,0],heading:0,target:[0,-200],dogs:[],people:[],peers:[{id:'a'.repeat(64),position:[500,0],form:'courier',age:0},{id:'b'.repeat(64),position:[-500,0],form:'dog',age:0}]}}));
  });
  await expect(page.locator('#world-loading')).toBeHidden();
  const direction=page.getByRole('img',{name:'Turn left · 24 m'});
  await expect(direction).toHaveText('24 m');
  await expect(direction).toHaveCSS('background-color','rgba(0, 0, 0, 0)');
  const left=await direction.locator('path').getAttribute('d');
  await page.evaluate(()=>dispatchEvent(new CustomEvent('paket-hud',{detail:{ready:true,routes:[],selected:0,interaction:false,minute:540,day:1,form:'courier',navigation:{turn:'right',distance:24}}})));
  await expect(page.getByRole('img',{name:'Turn right · 24 m'}).locator('path')).not.toHaveAttribute('d',left!);
  await page.evaluate(()=>dispatchEvent(new CustomEvent('paket-hud',{detail:{ready:true,routes:[],selected:0,interaction:true,minute:540,day:1,form:'courier',navigation:{turn:'right',distance:24}}})));
  await expect(page.locator('#route-direction')).toBeHidden();
  await expect(page.locator('#peer-list li')).toHaveCount(2);
  await expect(page.locator('#peer-list')).toContainText('Alice');
  const markers=page.locator('[data-peer-marker]');await expect(markers).toHaveCount(2);
  const colors=await markers.evaluateAll(nodes=>nodes.map(n=>n.getAttribute('data-color')));expect(new Set(colors).size).toBe(2);
  const rows=await page.locator('#peer-list li').evaluateAll(nodes=>nodes.map(n=>n.getAttribute('data-color')));expect(rows).toEqual(colors);
  await expect(page.locator('#target-marker')).toHaveAttribute('data-kind','delivery');
  await expect(page.locator('#target-marker')).toHaveAttribute('transform','translate(240 32)');
  await page.screenshot({path:test.info().outputPath('players.png')});
  await page.evaluate(()=>dispatchEvent(new CustomEvent('paket-radar',{detail:{position:[0,0],heading:0,target:null,dogs:[],people:[],peers:[]}})));
  await expect(markers).toHaveCount(0);await expect(page.locator('#peer-list li')).toHaveCount(2);
  await expect(page.locator('#peer-list')).toContainText('Waiting for position');
  await page.evaluate(()=>dispatchEvent(new CustomEvent('paket-members',{detail:[]})));
  await expect(page.locator('#peer-list li')).toHaveCount(0);
  await expect(page.locator('#target-marker')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('Given loaded WASM and a renderer still starting, the loading indicator remains until map readiness',async({page})=>{
  await page.addInitScript(()=>Object.defineProperty(navigator,'gpu',{value:{requestAdapter:async()=>({})}}));
  await page.route('**/game/paket_game.js',r=>r.fulfill({contentType:'text/javascript',body:'export default async()=>{}'}));
  await page.route('**/api/meshes',r=>r.fulfill({contentType:'application/json',body:'[]'}));
  await page.goto('/');await page.locator('#play').click();
  await expect(page.locator('#start')).toBeHidden();
  await expect(page.locator('#world-loading')).toBeVisible();
  await expect(page.locator('#world-loading-text')).toHaveText('Starting renderer…');
  await page.evaluate(()=>{
    dispatchEvent(new CustomEvent('paket-mode',{detail:'map'}));
    dispatchEvent(new CustomEvent('paket-map-loading',{detail:{ready:true,pending:false,failed:false}}));
  });
  await expect(page.locator('#world-loading')).toBeHidden();
});

test('the invitation form is visible on opening a narrow lobby and shows join errors next to the link', async ({page}) => {
  await page.setViewportSize({width:390,height:700});
  await page.addInitScript(()=>Object.defineProperty(navigator,'gpu',{value:{requestAdapter:async()=>({})}}));
  await page.route('**/api/meshes',r=>r.fulfill({json:[]}));
  await page.route('**/game/paket_game.js',r=>r.fulfill({contentType:'text/javascript',body:`
    export default async () => {
      dispatchEvent(new CustomEvent('paket-mode',{detail:'map'}));
      document.querySelector('#join-form').addEventListener('submit', () => {
        dispatchEvent(new CustomEvent('paket-lobby',{detail:{
          lat:52.4895,lon:13.3908,label:'Berlin',pending:false,ready:false,status:'Invalid invitation link.'
        }}));
      });
    }
  `}));
  await page.goto('/');
  await page.locator('#play').click();
  const link = page.getByRole('textbox',{name:'Join with an invitation link'});
  const join = page.getByRole('button',{name:'Join shift',exact:true});
  await expect(link).toBeInViewport();
  await expect(join).toBeInViewport();
  await link.fill('https://example.test/#join=invalid');
  await join.click();
  await expect(page.locator('#join-form').getByRole('status')).toHaveText('Invalid invitation link.');
  expect(await page.evaluate(()=>globalThis.paketCommands.some(command=>
    command.action==='join' && command.link==='https://example.test/#join=invalid'
  ))).toBe(true);
});

test('scene resolution starts sharp, adapts to slow frames, and is remembered', async ({page}) => {
  await page.addInitScript(()=>Object.defineProperty(navigator,'gpu',{value:{requestAdapter:async()=>({})}}));
  await page.route('**/game/paket_game.js',r=>r.fulfill({contentType:'text/javascript',body:`
    export default async () => {
      dispatchEvent(new CustomEvent('paket-mode',{detail:'ride'}));
      dispatchEvent(new CustomEvent('paket-hud',{detail:{ready:true,routes:[],form:'van'}}));
    }
  `}));
  await page.goto('/');
  await page.locator('#play').click();
  const canvas=page.locator('#game');
  await expect(canvas).toHaveAttribute('data-render-scale','1');
  await page.evaluate(()=>{for(let i=0;i<180;i++)globalThis.paketFrame(1000/60);});
  await expect(canvas).toHaveAttribute('data-render-scale','1');
  await page.evaluate(()=>{for(let i=0;i<130;i++)globalThis.paketFrame(40);});
  await expect.poll(()=>canvas.getAttribute('data-render-scale')).not.toBe('1');
  await page.evaluate(()=>{for(let i=0;i<240;i++)globalThis.paketFrame(1000/60);});
  const calibrated=await canvas.getAttribute('data-render-scale');
  await page.reload();
  await page.locator('#play').click();
  await expect(canvas).toHaveAttribute('data-render-scale',calibrated!);
});

test('a failed invite opens the lobby with the same address ready to retry', async ({page}) => {
  await page.addInitScript(()=>Object.defineProperty(navigator,'gpu',{value:{requestAdapter:async()=>({})}}));
  await page.route('**/api/meshes',r=>r.fulfill({json:[]}));
  await page.route('**/game/paket_game.js',r=>r.fulfill({contentType:'text/javascript',body:`
    export default async () => {
      dispatchEvent(new CustomEvent('paket-mode',{detail:'map'}));
      const lobby=(pending,ready,status)=>dispatchEvent(new CustomEvent('paket-lobby',{detail:{
        lat:0,lon:0,label:'Test street',pending,ready,status
      }}));
      lobby(true,false,'Connecting to mesh…');
      setTimeout(()=>lobby(false,false,'Could not join. Check that the host is online and try again.'),40);
      document.querySelector('#join-form').addEventListener('submit',()=> {
        lobby(true,false,'Connecting to mesh…');
        setTimeout(()=>lobby(false,true,'Mesh ready.'),40);
      });
    }
  `}));
  const link=new URL('/#join=an-invitation',test.info().project.use.baseURL).href;
  await page.goto(link);
  await page.locator('#play').click();
  const field=page.getByRole('textbox',{name:'Join with an invitation link'});
  await expect(field).toHaveValue(link);
  await expect(page.locator('#join-status')).toContainText('Could not join');
  await page.getByRole('button',{name:'Join shift',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>globalThis.paketCommands.map(c=>c.action))).toEqual(['join','join','start']);
});
