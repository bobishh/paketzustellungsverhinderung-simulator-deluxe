import { signEnvelope, verifyEnvelope, verifyDeviceCertificateChain } from '@meta-uber/mesh-identity';
import { profile, certificates } from './identity';
const DOMAIN='PAKET-PROFILE/1';
type Stats={workMs:number;shifts:number;delivered:number;photos:number;score:number;rings:number;markedVans:number;special:number};
type Career={kind:'paket-career';application:string;personId:string;deviceId:string;version:number;joinedAt:number;face:number;faceUpdated:number;stats:Stats};
type CareerRecord={identity:typeof profile.identity;certificates:typeof certificates;signed:Awaited<ReturnType<typeof signEnvelope<Career>>>};
const zero=():Stats=>({workMs:0,shifts:0,delivered:0,photos:0,score:0,rings:0,markedVans:0,special:0});
const records=new Map<string,CareerRecord>();
let db:IDBDatabase,local:Career,dirty=false,saving=false,initialized=false,previous=zero(),playing=false,lastTick=0;
const id=(p:Career)=>`${p.personId}/${p.deviceId}`;
async function store(record:CareerRecord){await new Promise<void>((resolve,reject)=>{const tx=db.transaction('profiles','readwrite');tx.objectStore('profiles').put(record,id(record.signed.payload));tx.oncomplete=()=>resolve();tx.onerror=()=>reject(tx.error);});}
export async function acceptProfile(record:CareerRecord,persist=true){
  if(!record||JSON.stringify(record).length>20000)throw new Error('Profile too large');
  const p=record.signed?.payload;
  if(!p||p.kind!=='paket-career'||p.application!==location.origin||p.personId!==record.identity?.personId||p.deviceId!==record.signed.signerKeyId||!Number.isSafeInteger(p.version)||p.version<1||!Number.isSafeInteger(p.face)||p.face<0||p.face>0xffffffff||!Number.isSafeInteger(p.joinedAt)||p.joinedAt<0||p.joinedAt>Date.now()+60000||!Number.isSafeInteger(p.faceUpdated)||p.faceUpdated<0||p.faceUpdated>Date.now()+60000||!p.stats||Object.keys(zero()).some(k=>!Number.isSafeInteger(p.stats[k as keyof Stats])||p.stats[k as keyof Stats]<0||p.stats[k as keyof Stats]>1e12))throw new Error('Invalid career');
  const existing=records.get(id(p));if(existing&&existing.signed.payload.version>=p.version)return;
  const key=await verifyDeviceCertificateChain(record.identity,p.deviceId,record.certificates);
  if(!await verifyEnvelope(record.signed,key,DOMAIN))throw new Error('Invalid career signature');
  if(existing&&Object.keys(zero()).some(k=>p.stats[k as keyof Stats]<existing.signed.payload.stats[k as keyof Stats]))throw new Error('Career counters cannot go backwards');
  if(!existing&&records.size>=512)throw new Error('Profile cache full');
  if(persist)await store(record);records.set(id(p),record);if(initialized)render();
}
export function profileBatch(cursor=0):CareerRecord[]{const all=[...records.values()];return all.length?[all[cursor%all.length]]:[];}
async function flush(){
  if(!initialized||!dirty||saving)return;saving=true;dirty=false;
  try{const data=structuredClone(local);data.version++;const signed=await signEnvelope(profile.privateKeys.devicePrivateKey,data,profile.device.deviceId,DOMAIN);await acceptProfile({identity:profile.identity,certificates,signed});local.version=data.version;}
  catch(e){dirty=true;throw e;}finally{saving=false;}
}
export async function initProfile(){
  db=await new Promise<IDBDatabase>((resolve,reject)=>{const r=indexedDB.open('paket.application-mesh',1);r.onupgradeneeded=()=>r.result.createObjectStore('profiles');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});
  const saved:CareerRecord[]=await new Promise((resolve,reject)=>{const r=db.transaction('profiles').objectStore('profiles').getAll();r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});
  for(const record of saved){try{await acceptProfile(record,false);}catch{/* Ignore unverifiable records. */}}
  const own=records.get(`${profile.identity.personId}/${profile.device.deviceId}`)?.signed.payload;
  const guestCareer=[...records.values()].map(r=>r.signed.payload).filter(p=>p.deviceId===profile.device.deviceId).sort((a,b)=>b.version-a.version)[0];
  local=own?structuredClone(own):guestCareer?{...structuredClone(guestCareer),personId:profile.identity.personId,version:0}:{kind:'paket-career',application:location.origin,personId:profile.identity.personId,deviceId:profile.device.deviceId,version:0,joinedAt:Date.now(),face:crypto.getRandomValues(new Uint32Array(1))[0],faceUpdated:Date.now(),stats:zero()};
  initialized=true;dirty=true;await flush();render();
  setInterval(()=>{void flush().catch(e=>paketNotify(`Profile save failed: ${e.message}`));},3000);
}
function career(){
  const own=[...records.values()].map(r=>r.signed.payload).filter(p=>p.personId===profile.identity.personId);
  const i=own.findIndex(p=>p.deviceId===local.deviceId);if(i>=0)own[i]=local;else own.push(local);
  const stats=zero();for(const p of own)for(const k of Object.keys(stats)as(keyof Stats)[])stats[k]+=p.stats[k];
  const face=own.slice().sort((a,b)=>b.faceUpdated-a.faceUpdated||a.deviceId.localeCompare(b.deviceId))[0]?.face??0;
  return{stats,face,joinedAt:Math.min(...own.map(p=>p.joinedAt))};
}
function portrait(seed:number){
  const skin=['#d29d78','#b77953','#865439','#e7b492'][seed%4],hair=['#29241d','#48362c','#aa7950','#84765e'][(seed>>>3)%4];
  return `<svg viewBox="0 0 120 140" width="120" height="140" role="img" aria-label="Generated courier portrait"><path fill="#ec853e" d="M12 140v-25q3-23 48-23t48 23v25"/><path fill="${skin}" d="M48 87h24v24H48z"/><rect x="30" y="25" width="60" height="72" rx="24" fill="${skin}"/><path fill="${hair}" d="M28 49V27q0-19 31-19t33 19v22l-12-23-42 8z"/><path fill="#282c25" d="M37 51h16v4H37zm30 0h16v4H67z"/><circle cx="46" cy="63" r="3" fill="#263b37"/><circle cx="75" cy="63" r="3" fill="#263b37"/><path d="M58 63v12h7m-17 9q13 7 25-1" fill="none" stroke="#835437" stroke-width="3"/><path fill="#19382b" d="M22 126h76v14H22z"/>${seed%3===0?'<g fill="none" stroke="#342b24" stroke-width="3"><circle cx="45" cy="63" r="12"/><circle cx="76" cy="63" r="12"/><path d="M57 62h7"/></g>':''}</svg>`;
}
function render(){
  if(!initialized||typeof document==='undefined')return;const c=career(),s=c.stats,days=Math.max(1,Math.floor((Date.now()-c.joinedAt)/86400000)+1);
  const content=document.querySelector('#profile-content');if(!content)return;
  content.innerHTML=`<div class="profile-top">${portrait(c.face)}<div><strong class="career-name"></strong><p>Employed for ${days} ${days===1?'day':'days'}</p><button id="new-face">New face ↻</button></div></div><div class="profile-stats"><div><b>${s.delivered}</b><small>Parcels completed</small></div><div><b>${s.score}</b><small>Career KPI</small></div><div><b>${Math.floor(s.workMs/60000)}m</b><small>Time on the job</small></div></div>`;
  content.querySelector('.career-name')!.textContent=document.querySelector('#identity-name')!.textContent??'Courier';
  for(const [title,done]of [['First day on the job',s.shifts>0],['Ring leader · 6 doorbells',s.rings>=6],['Photographic evidence',s.photos>0],['Territory claimed',s.markedVans>0],['Delivery of the day',s.special>0],['1000 KPI club',s.score>=1000]]as const){const el=document.createElement('div');el.className=`achievement${done?'':' locked'}`;el.textContent=`${done?'★':'○'}  ${title}`;content.append(el);}
  const sync=document.createElement('p');sync.className='eyebrow';sync.textContent='Game profile · signed by your Identity · syncs with connected players';content.append(sync);
  content.querySelector('#new-face')!.addEventListener('click',()=>{local.face=crypto.getRandomValues(new Uint32Array(1))[0];local.faceUpdated=Date.now();dirty=true;render();void flush().catch(e=>paketNotify(e.message));});
}
if(typeof document!=='undefined'){
  const dialog=document.querySelector<HTMLDialogElement>('#profile-dialog');
  document.querySelector('#player-profile')?.addEventListener('click',()=>{if(document.pointerLockElement)void document.exitPointerLock();if(!initialized){paketNotify('Profile is still loading.');return;}render();dialog?.showModal();});
  dialog?.querySelector('.dialog-close')?.addEventListener('click',()=>dialog.close());
  window.addEventListener('paket-mode',((e:CustomEvent<string>)=>{playing=e.detail==='ride';previous=zero();lastTick=Date.now();if(playing&&initialized){local.stats.shifts++;dirty=true;}})as EventListener);
  window.addEventListener('paket-progress',((e:CustomEvent<Stats&{ended:boolean}>)=>{
    if(!initialized||!playing)return;const now=Date.now(),p=e.detail;
    if(!p.ended)local.stats.workMs+=Math.min(1000,Math.max(0,now-lastTick));lastTick=now;
    for(const k of ['delivered','photos','score','rings','markedVans','special']as const){const n=Number(p[k]);if(Number.isFinite(n)&&n>=0){local.stats[k]+=Math.max(0,n-previous[k]);previous[k]=n;}}
    dirty=true;
  })as EventListener);
  document.addEventListener('visibilitychange',()=>{if(document.hidden)void flush().catch(()=>undefined);});
}
