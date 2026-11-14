import {peerColor,readMembers,ringMarker,type Point,type Peer,type Member} from './peers';
type Road = { points: Point[]; width: number };
type Radar = { position: Point; heading: number; target?: Point; van?: Point; dogs: Point[]; people: Point[]; peers: Peer[] };
type Stop = {index:number;address:string;name:string;point:Point;state:string};
let roads: Road[] = [];
let radar:Radar|undefined, stops:Stop[]=[], selected=0, riding=false, interaction=false;
let centre:Point=[0,0], halfWidth=250;
const mapDialog=document.querySelector<HTMLDialogElement>('#route-map-dialog')!;
const routeCanvas=document.querySelector<HTMLCanvasElement>('#route-map')!;
const routeCtx=routeCanvas.getContext('2d')!;
const game=document.querySelector<HTMLCanvasElement>('#game')!;
const send=(command:{action:string;[key:string]:unknown})=>globalThis.paketCommands.push(command);
function fitRoute(){
  if(!radar)return;
  const points=[radar.position,...stops.map(s=>s.point),...(radar.van?[radar.van]:[])];
  const xs=points.map(p=>p[0]),ys=points.map(p=>p[1]);
  const minX=Math.min(...xs),maxX=Math.max(...xs),minY=Math.min(...ys),maxY=Math.max(...ys);
  centre=[(minX+maxX)/2,(minY+maxY)/2];
  halfWidth=Math.max(100,(maxX-minX)/2+45,((maxY-minY)/2+45)*1000/700);
  drawRoute();
}
function drawRoute(){
  if(!mapDialog.open||!radar)return;
  const c=routeCtx,scale=1000/(halfWidth*2);
  const xy=(p:Point):Point=>[500+(p[0]-centre[0])*scale,350+(p[1]-centre[1])*scale];
  c.fillStyle='#233b2e';c.fillRect(0,0,1000,700);
  c.lineCap='round';c.lineJoin='round';
  for(const road of roads){c.beginPath();road.points.forEach((p,i)=>{const [x,y]=xy(p);if(i)c.lineTo(x,y);else c.moveTo(x,y);});c.lineWidth=Math.max(2,road.width*scale);c.strokeStyle='#778774';c.stroke();}
  const dot=(p:Point,color:string,size:number,square=false)=>{const [x,y]=xy(p);c.fillStyle=color;c.strokeStyle='#13251d';c.lineWidth=3;c.beginPath();if(square)c.rect(x-size,y-size,size*2,size*2);else c.arc(x,y,size,0,Math.PI*2);c.fill();c.stroke();};
  for(const p of radar.people)dot(p,'#a7ddca',5);
  for(const p of radar.dogs)dot(p,'#efba76',4);
  for(const p of radar.peers)dot(p.position,colorFor(p.id),7);
  if(radar.van)dot(radar.van,'#ffdb84',10,true);
  for(const stop of stops){const done=['filed','handed'].includes(stop.state);dot(stop.point,stop.index===selected?'#ff8436':done?'#779783':'#efe5d0',17);const [x,y]=xy(stop.point);c.fillStyle='#14271e';c.font='bold 18px system-ui';c.textAlign='center';c.textBaseline='middle';c.fillText(String(stop.index+1),x,y);}
  const [x,y]=xy(radar.position);c.save();c.translate(x,y);c.rotate(-radar.heading);c.beginPath();c.moveTo(0,-18);c.lineTo(12,12);c.lineTo(0,5);c.lineTo(-12,12);c.closePath();c.fillStyle='#fff9e8';c.fill();c.strokeStyle='#102419';c.lineWidth=3;c.stroke();c.restore();
  c.fillStyle='#fff9e8';c.font='bold 20px system-ui';c.textAlign='left';c.fillText('N ↑',20,28);
  document.querySelector('#route-map-scale')!.textContent=`${Math.round(halfWidth*2)} m across`;
  const stop=stops.find(s=>s.index===selected);
  document.querySelector('#route-map-stop')!.textContent=stop?`${stop.index+1} · ${stop.address} · ${stop.name}`:'Streets and deliveries appear as the neighbourhood loads.';
}
function openMap(){
  if(!riding||interaction||document.querySelector('dialog[open]'))return;
  send({action:'menu',open:true});globalThis.paketWantsPointer=false;void document.exitPointerLock();mapDialog.showModal();fitRoute();
}
function closeMap(){mapDialog.close();}
document.querySelector<HTMLButtonElement>('#hud-map')!.onclick=openMap;
document.querySelector<HTMLButtonElement>('#route-map-close')!.onclick=closeMap;
document.querySelector<HTMLButtonElement>('#route-map-fit')!.onclick=fitRoute;
document.querySelector<HTMLButtonElement>('#route-map-in')!.onclick=()=>{halfWidth=Math.max(50,halfWidth/1.4);drawRoute();};
document.querySelector<HTMLButtonElement>('#route-map-out')!.onclick=()=>{halfWidth=Math.min(5000,halfWidth*1.4);drawRoute();};
routeCanvas.addEventListener('click',event=>{
  const b=routeCanvas.getBoundingClientRect(),x=(event.clientX-b.left)/b.width*1000,y=(event.clientY-b.top)/b.height*700,scale=1000/(halfWidth*2);
  const stop=stops.map(s=>({s,d:Math.hypot(500+(s.point[0]-centre[0])*scale-x,350+(s.point[1]-centre[1])*scale-y)})).sort((a,b)=>a.d-b.d)[0];
  if(stop&&stop.d<28){send({action:'delivery',index:stop.s.index});selected=stop.s.index;drawRoute();}
});
mapDialog.addEventListener('close',()=>{if(riding){send({action:'menu',open:false});game.focus({preventScroll:true});}});
window.addEventListener('keydown',event=>{
  if(!riding||event.code!=='KeyM'||event.metaKey||event.ctrlKey||event.altKey)return;
  if(mapDialog.open){event.preventDefault();event.stopImmediatePropagation();if(!event.repeat)closeMap();return;}
  if(document.querySelector('dialog[open]')||/INPUT|TEXTAREA|SELECT/.test((event.target as HTMLElement).tagName))return;
  event.preventDefault();event.stopImmediatePropagation();if(!event.repeat)openMap();
},true);
window.addEventListener('paket-mode',((event:CustomEvent<string>)=>{riding=event.detail==='ride';if(mapDialog.open)mapDialog.close();radar=undefined;stops=[];roads=[];})as EventListener);
window.addEventListener('paket-hud',((event:CustomEvent<{routes:Stop[];selected:number;interaction:boolean}>)=>{stops=event.detail.routes.filter(s=>s.point?.length===2);selected=event.detail.selected;interaction=event.detail.interaction;drawRoute();})as EventListener);
window.addEventListener('paket-day',((event:CustomEvent<{day:number}>)=>{globalThis.paketNotify(`Day ${event.detail.day} · 09:00. Fresh parcels in your van.`);})as EventListener);
const canvas = document.querySelector<HTMLCanvasElement>('#radar')!;
const ctx = canvas.getContext('2d')!;
window.addEventListener('paket-map-roads', ((e: CustomEvent<Road[]>) => { roads = e.detail; }) as EventListener);
window.addEventListener('paket-radar', ((e: CustomEvent<{ position: Point; heading: number; target?: Point; van?: Point; dogs: Point[]; people: Point[]; peers: Peer[] }>) => {
  const s = radar = e.detail, half = 80, scale = 480 / (half * 2);
  drawRoute();
  const xy = (p: Point): Point => [240 + (p[0] - s.position[0]) * scale, 240 + (p[1] - s.position[1]) * scale];
  ctx.clearRect(0, 0, 480, 480); ctx.fillStyle = '#233b2e'; ctx.fillRect(0, 0, 480, 480);
  ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  for (const road of roads) {
    ctx.beginPath(); road.points.forEach((p, i) => { const [x,y] = xy(p); if (i) ctx.lineTo(x,y); else ctx.moveTo(x,y); });
    ctx.lineWidth = Math.max(3, road.width * scale); ctx.strokeStyle = '#7d8775'; ctx.stroke();
  }
  const dot = (p: Point, color: string, size: number, square = false, clamp = false) => {
    let [x,y] = xy(p); if(clamp){x=Math.max(12,Math.min(468,x));y=Math.max(12,Math.min(468,y));}
    ctx.fillStyle = color;ctx.strokeStyle='#17231e';ctx.lineWidth=3;ctx.beginPath();
    if(square)ctx.rect(x-size,y-size,size*2,size*2);else ctx.arc(x,y,size,0,Math.PI*2);ctx.fill();ctx.stroke();
  };
  for(const p of s.dogs)dot(p,'#efba76',5);
  for(const p of s.people)dot(p,'#a7ddca',6);
  for(const p of s.peers)dot(p.position,colorFor(p.id),7);
  drawPlayers(s);
  if(s.van)dot(s.van,'#ffdb84',9,true,true);

  ctx.save();ctx.translate(240,240);ctx.rotate(-s.heading);ctx.beginPath();ctx.moveTo(0,-16);ctx.lineTo(11,11);ctx.lineTo(0,5);ctx.lineTo(-11,11);ctx.closePath();ctx.fillStyle='#fff9e8';ctx.fill();ctx.strokeStyle='#102419';ctx.lineWidth=3;ctx.stroke();ctx.restore();
  ctx.beginPath();ctx.arc(240,240,90*scale/2,0,Math.PI*2);ctx.strokeStyle='#ffffff20';ctx.lineWidth=1;ctx.stroke();
}) as EventListener);

const members=new Map<string,Member>();
let localId:string|undefined;
const rows=new Map<string,HTMLLIElement>(), markers=new Map<string,SVGGElement>();
const layer=document.querySelector<SVGSVGElement>('#radar-markers')!;
const list=document.querySelector<HTMLUListElement>('#peer-list')!;
const svg=(tag:string)=>document.createElementNS('http://www.w3.org/2000/svg',tag);
const memberFor=(id:string)=>members.get(id)??{id,name:`Player ${id.slice(0,6)}`,slot:parseInt(id.slice(0,4),16)%17};
const colorFor=(id:string)=>peerColor(memberFor(id).slot);
let targetMarker:SVGGElement|undefined;
function marker(target=false){
  const group=svg('g') as SVGGElement;
  const arrow=svg('path');arrow.setAttribute('d',target?'M-9 -22 L0 -34 L9 -22':'M-6 -15 L0 -25 L6 -15');arrow.setAttribute('class','bearing');group.append(arrow);
  const shape=svg(target?'path':'circle');
  if(target)shape.setAttribute('d','M0 -22 L22 0 L0 22 L-22 0Z');else shape.setAttribute('r','13');
  shape.setAttribute('stroke','#14251f');shape.setAttribute('stroke-width','4');group.append(shape);
  const text=svg('text');text.setAttribute('text-anchor','middle');text.setAttribute('y','6');text.setAttribute('fill','#14251f');text.setAttribute('font-size',target?'26':'18');text.setAttribute('font-weight','800');group.append(text);
  const title=svg('title');group.append(title);layer.append(group);return group;
}
function place(group:SVGGElement,position:Point,origin:Point){
  const p=ringMarker(origin,position);group.setAttribute('transform',`translate(${p.x} ${p.y})`);
  const arrow=group.querySelector('.bearing')!;arrow.setAttribute('transform',`rotate(${p.angle})`);arrow.setAttribute('visibility',p.onRing?'visible':'hidden');
  return p.distance;
}
function drawPlayers(s:Radar){
  const positions=new Map(s.peers.map(p=>[p.id,p]));
  const present=new Set([...members.keys(),...positions.keys()].filter(id=>id!==localId));
  for(const [id,row] of rows)if(!present.has(id)){row.remove();rows.delete(id);}
  for(const [id,mark] of markers)if(!positions.has(id)){mark.remove();markers.delete(id);}
  for(const id of [...present].sort((a,b)=>memberFor(a).slot-memberFor(b).slot)){
    const member=memberFor(id),color=peerColor(member.slot),p=positions.get(id);
    let row=rows.get(id);
    if(!row){row=document.createElement('li');row.innerHTML='<span class="peer-badge"></span><span class="peer-name"></span><small></small>';rows.set(id,row);list.append(row);}
    row.dataset.color=color;row.style.setProperty('--peer-color',color);
    const number=String(member.slot+1);
    row.querySelector('.peer-badge')!.textContent=number;row.querySelector('.peer-name')!.textContent=member.name;
    row.querySelector('small')!.textContent='Waiting for position…';
    if(p){
      let mark=markers.get(id);
      if(!mark){mark=marker();mark.setAttribute('data-peer-marker',id);markers.set(id,mark);}
      mark.dataset.color=color;mark.setAttribute('fill',color);
      const distance=place(mark,p.position,s.position);
      row.querySelector('small')!.textContent=p.age>3?'Reconnecting…':`${p.form} · ${Math.round(distance)} m`;
      mark.querySelector('text')!.textContent=number;mark.querySelector('title')!.textContent=`${member.name}: ${Math.round(distance)} m`;
      mark.setAttribute('opacity',p.age>3?'0.5':'1');
    }
  }
  document.querySelector('#peer-count')!.textContent=String(present.size);
  document.querySelector<HTMLElement>('#peer-empty')!.hidden=present.size>0;
  if(s.target){
    if(!targetMarker){targetMarker=marker(true);targetMarker.id='target-marker';targetMarker.dataset.kind='delivery';targetMarker.setAttribute('fill','#ff943f');targetMarker.querySelector('text')!.textContent='◆';}
    const distance=place(targetMarker,s.target,s.position);targetMarker.querySelector('title')!.textContent=`Delivery: ${Math.round(distance)} m`;
    layer.append(targetMarker);
  }else{targetMarker?.remove();targetMarker=undefined;}
}
window.addEventListener('paket-members',((e:CustomEvent<unknown>)=>{members.clear();for(const m of readMembers(e.detail))members.set(m.id,m);if(radar)drawPlayers(radar);})as EventListener);
window.addEventListener('paket-session',((e:CustomEvent)=>{localId=e.detail?.device;if(e.detail===null){members.clear();if(radar)drawPlayers({...radar,peers:[]});}})as EventListener);
export {};
