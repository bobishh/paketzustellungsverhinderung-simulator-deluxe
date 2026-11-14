import {test,expect} from '@playwright/test';

test('Given a real mesh peer, the WASM game interpolates sparse poses and expires disconnected players',async({browser})=>{
  test.skip(!process.env.PAKET_REAL_GAME,'Enable with npm run test:game; requires WebGPU and Iroh relay access.');
  test.setTimeout(180_000);
  const hostContext=await browser.newContext(),guestContext=await browser.newContext({viewport:{width:640,height:480}});
  const host=await hostContext.newPage(),guest=await guestContext.newPage();
  const errors:string[]=[];
  guest.on('pageerror',e=>{if(!e.message.includes('Using exceptions for control flow'))errors.push(e.message)});
  await guest.addInitScript(()=>{
    const w=globalThis as any;w.samples=[];w.hudFrames=0;
    addEventListener('paket-radar',(e:any)=>{w.radar=e.detail;if(w.record&&e.detail.peers[0])w.samples.push({at:performance.now(),p:e.detail.peers[0].position});});
    addEventListener('paket-hud',()=>w.hudFrames++);
  });
  const ll=(east:number,north:number)=>({lat:north/6378137*180/Math.PI,lon:east/6378137*180/Math.PI});
  const fixture={elements:[{type:'way',id:1,tags:{highway:'residential',name:'Test Street'},geometry:[ll(-100,0),ll(100,0)]},
    ...[-40,0,40].map((x,i)=>({type:'way',id:10+i,tags:{building:'yes','building:levels':'2','addr:street':'Test Street','addr:housenumber':String(i+1)},geometry:[ll(x-6,10),ll(x+6,10),ll(x+6,20),ll(x-6,20),ll(x-6,10)]}))]};
  await guest.route('**/api/osm?*',r=>r.fulfill({contentType:'application/json',body:JSON.stringify(fixture)}));
  try{
    // The producer uses the actual identity and Iroh transport. A controlled clock
    // isolates network interpolation from the producer's GPU/frame scheduling.
    await host.goto('/');await host.waitForFunction(()=>localStorage.getItem('paket.identity'));
    const invitation=await host.evaluate(async()=>{
      const w=globalThis as any,origin={lat:0,lon:0};
      const session=JSON.parse(await w.paketMesh.connect(origin));w.sent=[];w.walkAt=0;
      let sequence=0;
      const publish=()=>{
        const now=performance.now(),east=w.walkAt?(now-w.walkAt)/1000*4:0;
        w.paketMesh.publish(session.device,{kind:'actor',origin,device:session.device,sequence:++sequence,pose:{avatar:'courier',east,north:0,up:0,heading:Math.PI/2,velocity_east:w.walkAt?4:0,velocity_north:0,parked_van:null}});
        w.paketMesh.publish(session.device,{kind:'route',origin,device:session.device,day:1,revision:2,stops:[
          {building_id:'way/10',label:'Mesh stop A',position:[-40,0,-10],completion:{device:session.device,photo:true,score:100}},
          {building_id:'way/11',label:'Mesh stop B',position:[0,0,-10],completion:null},
          {building_id:'way/12',label:'Mesh stop C',position:[40,0,-10],completion:null},
        ]});
        w.paketMesh.publish(session.device,{kind:'shift',origin,device:session.device,elapsed_ms:0,difficulty:1,day:1});
        w.sent.push({at:now,east});
      };
      publish();w.poseTimer=setInterval(publish,300);return JSON.parse(session.invitation);
    });
    const link=`http://127.0.0.1:8797/#join=${Buffer.from(JSON.stringify(invitation)).toString('base64url')}`;
    await guest.goto(link);await guest.locator('#play').click();
    await expect(guest.locator('#peer-list li')).toHaveCount(1,{timeout:90_000});
    await expect(guest.locator('[data-peer-marker]')).toHaveCount(1);
    await expect(guest.locator('#peer-list')).toContainText('Courier');
    const color=await guest.locator('#peer-list li').getAttribute('data-color');
    await expect(guest.locator('[data-peer-marker]')).toHaveAttribute('data-color',color!);
    await expect(guest.locator('#world-loading')).toBeHidden({timeout:30_000});
    await expect(guest.locator('a:visible')).toHaveCount(0);
    await expect(guest.locator('#target-marker')).toBeVisible({timeout:30_000});
    await expect(guest.locator('#route-total')).toHaveText('1 / 3');
    await expect(guest.locator('#stop-address')).toHaveText('Mesh stop B');
    await expect(guest.locator('#hud-score')).toHaveText('0');
    await guest.locator('#route-toggle').click();
    await expect(guest.locator('#route-list .route-stop').first()).toContainText('Delivered');
    await guest.locator('#route-toggle').click();
    await guest.locator('#share').click();await expect(guest.locator('#share-link')).toHaveValue(link);await guest.locator('#share-close').click();
    await expect(guest.locator('#share-dialog')).toBeHidden();
    await guest.waitForFunction(()=>(globalThis as any).hudFrames>120,{},{timeout:60_000});
    await host.evaluate(()=>{(globalThis as any).walkAt=performance.now();});
    await guest.waitForTimeout(2500);
    await guest.evaluate(()=>{(globalThis as any).record=true;});
    await guest.waitForFunction(()=>(globalThis as any).samples.length>=45,{},{timeout:20_000});
    const samples=await guest.evaluate(()=>(globalThis as any).samples as {at:number;p:number[]}[]);
    const steps=samples.slice(1).map((s,i)=>({dt:(s.at-samples[i].at)/1000,dx:s.p[0]-samples[i].p[0]}));
    await test.info().attach('remote-motion',{body:JSON.stringify({samples,steps,sent:await host.evaluate(()=>(globalThis as any).sent)}),contentType:'application/json'});
    expect(samples.length).toBeGreaterThan(30);
    const regular=steps.filter(s=>s.dt<0.25);
    expect(regular.length).toBeGreaterThan(25);
    expect(samples.at(-1)!.p[0]-samples[0].p[0]).toBeGreaterThan(12);
    expect(Math.max(...regular.map(s=>s.dx))).toBeLessThan(1.8);
    expect(regular.filter(s=>s.dx<0.02).length/regular.length).toBeLessThan(0.1);
    await guest.screenshot({path:test.info().outputPath('mesh-players.png')});
    await hostContext.close();
    await expect(guest.locator('#peer-list li')).toHaveCount(0,{timeout:20_000});
    await expect(guest.locator('[data-peer-marker]')).toHaveCount(0);
    await guest.waitForTimeout(1500);await expect(guest.locator('#peer-list li')).toHaveCount(0);
    expect(errors).toEqual([]);
  }finally{await guestContext.close();await hostContext.close();}
});
