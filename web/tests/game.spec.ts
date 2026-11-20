import {observeGame,clickUi,fact,hud,selected,waitRide} from './bevy-ui';
import {test,expect} from '@playwright/test';

test('Given a real mesh peer, the WASM game interpolates sparse poses and expires disconnected players',async({browser})=>{
  test.skip(!process.env.PAKET_REAL_GAME,'Enable with npm run test:game; requires WebGPU and Iroh relay access.');
  test.setTimeout(180_000);
  const hostContext=await browser.newContext(),guestContext=await browser.newContext({viewport:{width:640,height:480}});
  const host=await hostContext.newPage(),guest=await guestContext.newPage();
  await observeGame(guest);
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
  await guest.route('**/elevation/**', r => r.fulfill({status:503}));
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
          {building_id:'way/10',label:'Mesh stop A',position:[-40,0,-10],completion:{device:session.device,photo:false,score:335}},
          {building_id:'way/11',label:'Mesh stop B',position:[0,0,-10],completion:null},
          {building_id:'way/12',label:'Mesh stop C',position:[40,0,-10],completion:null},
        ]});
        w.paketMesh.publish(session.device,{kind:'shift',origin,device:session.device,elapsed_ms:0,difficulty:1,day:1});
        w.sent.push({at:now,east});
      };
      publish();w.poseTimer=setInterval(publish,300);return JSON.parse(session.invitation);
    });
    const link=new URL('/#join='+Buffer.from(JSON.stringify(invitation)).toString('base64url'),host.url()).href;
    await guest.goto(link);await waitRide(guest);
    await expect.poll(async()=>(await fact(guest,'paket-radar'))?.peers?.length,{timeout:90_000}).toBe(1);
    await expect.poll(async()=>(await fact(guest,'paket-ui-regions'))?.texts?.some((t:any)=>t.text.includes('Courier'))).toBe(true);
    expect((await hud(guest)).delivered).toBe(1);
    expect((await hud(guest)).routes).toHaveLength(3);
    expect((await selected(guest)).address).toBe('Mesh stop B');expect((await hud(guest)).score).toBe(0);
    await clickUi(guest,'Deliveries');await clickUi(guest,/1 · Mesh stop A · handed/);await clickUi(guest,/2 · Mesh stop B ·/);await clickUi(guest,'Close · Esc');
    await clickUi(guest,'Share mesh');
    await expect.poll(async()=>(await fact(guest,'paket-ui-regions'))?.regions?.some((r:any)=>r.label==='Invitation link'&&r.value===link)).toBe(true);
    await clickUi(guest,'Close · Esc');
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
    // Match the actual drawn roster and minimap, rather than a DOM reconstruction.
    await expect.poll(async()=>{
      const view=await fact(guest,'paket-ui-regions');
      const peer=view?.markers?.find((mark:any)=>mark.kind==='peer');
      return Boolean(peer&&view.markers.some((mark:any)=>mark.kind==='roster'&&mark.id===peer.id));
    }).toBe(true);
    const view=await fact(guest,'paket-ui-regions');
    const peer=view.markers.find((mark:any)=>mark.kind==='peer');
    const row=view.markers.find((mark:any)=>mark.kind==='roster'&&mark.id===peer.id);
    const ring=view.markers.find((mark:any)=>mark.kind==='ring');
    const target=view.markers.find((mark:any)=>mark.kind==='target');
    expect(peer.color).toEqual(row.color);expect(peer.slot).toBe(row.slot);
    const member=(await fact(guest,'paket-members')).find((member:any)=>member.id===peer.id);
    expect(view.texts.some((item:any)=>item.text===`${peer.slot+1} · ${member.name}`)).toBe(true);
    expect(view.texts.some((item:any)=>item.text===String(peer.slot+1))).toBe(true);
    expect(ring).toBeDefined();
    expect(Math.hypot(peer.x+peer.width/2-ring.x-ring.width/2,peer.y+peer.height/2-ring.y-ring.height/2)).toBeCloseTo(ring.width/2,0);
    expect(target).toBeDefined();expect(target.width).toBeGreaterThan(peer.width);
    expect(target.color).not.toEqual(peer.color);
    await guest.screenshot({path:test.info().outputPath('mesh-players.png')});
    await hostContext.close();
    await expect.poll(async()=>(await fact(guest,'paket-radar'))?.peers?.length,{timeout:20_000}).toBe(0);
    await guest.waitForTimeout(1500);expect((await fact(guest,'paket-radar')).peers).toHaveLength(0);
    await expect.poll(async()=>((await fact(guest,'paket-ui-regions'))?.markers??[]).filter((mark:any)=>mark.kind==='peer').length).toBe(0);
    expect(errors).toEqual([]);
  }finally{await guestContext.close();await hostContext.close();}
});

test('a host can reopen the same mesh with the same invitation and completed stops',async({browser})=>{
  test.skip(!process.env.PAKET_REAL_GAME,'Requires Iroh relay access and WebGPU.');
  const hostContext=await browser.newContext(),guestContext=await browser.newContext();
  try{
    const origin={lat:52.4895,lon:13.3908};
    const road={type:'way',id:1,tags:{highway:'residential',name:'Test Street'},geometry:[origin,{...origin,lon:origin.lon+0.001}]};
    for(const context of [hostContext,guestContext]){
      await context.route('**/api/osm?*',r=>r.fulfill({json:{elements:[road]}}));
      await context.route('**/elevation/**',r=>r.fulfill({status:503}));
    }
    const host=await hostContext.newPage();
    await host.goto('/');
    await host.waitForFunction(()=>localStorage.getItem('paket.identity'));
    const session=await host.evaluate(async()=>{
      const origin={lat:52.4895,lon:13.3908};
      const room=JSON.parse(await globalThis.paketMesh.connect(origin));
      const shared={origin,device:room.device,day:1};
      globalThis.paketMesh.publish(room.device,{...shared,kind:'shift',elapsed_ms:90000,difficulty:1});
      globalThis.paketMesh.publish(room.device,{...shared,kind:'route',revision:7,resident_seed:12345,stops:[
        {building_id:'way/10',label:'Resumed stop A',position:[-40,0,-10],completion:{device:room.device,photo:false,score:335}},
        {building_id:'way/11',label:'Resumed stop B',recipient:'Mila Weber',position:[0,0,-10],completion:null},
      ]});
      return room;
    });
    const link=new URL('/#join='+Buffer.from(session.invitation).toString('base64url'),host.url()).href;
    await host.close();
    const reopened=await hostContext.newPage();
    await observeGame(reopened);await reopened.goto('/');
    await clickUi(reopened,'Play');await clickUi(reopened,'Friends');await clickUi(reopened,'Return to your mesh');await waitRide(reopened);
    await expect.poll(async()=>(await hud(reopened)).delivered,{timeout:80_000}).toBe(1);
    expect((await hud(reopened)).routes).toHaveLength(2);expect((await hud(reopened)).score).toBe(335);
    expect((await selected(reopened)).name).toBe('Mila Weber');
    await clickUi(reopened,'Settings');await clickUi(reopened,'Change neighbourhood');
    await clickUi(reopened,'Friends');await clickUi(reopened,'Return to your mesh');await waitRide(reopened);
    expect((await hud(reopened)).delivered).toBe(1);expect((await hud(reopened)).score).toBe(335);
    const guest=await guestContext.newPage();await observeGame(guest);await guest.goto(link);await waitRide(guest);
    expect((await hud(guest)).delivered).toBe(1);expect((await hud(guest)).routes).toHaveLength(2);
    expect((await selected(guest)).name).toBe('Mila Weber');expect((await hud(guest)).score).toBe(0);
  }finally{await guestContext.close();await hostContext.close();}
});
