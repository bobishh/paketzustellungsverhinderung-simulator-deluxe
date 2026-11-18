import { signEnvelope, verifyEnvelope, verifyDeviceCertificateChain } from '@meta-uber/mesh-identity';
import {profile,certificates,identityName,identityLinked} from './identity';
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
export function emitProfile(){
  if(!initialized)return;
  const c=career();
  globalThis.dispatchEvent(new CustomEvent('paket-profile',{detail:{...c,name:identityName||'Courier',linked:identityLinked}}));
}
export function newFace(){
  if(!initialized)return;
  local.face=crypto.getRandomValues(new Uint32Array(1))[0];local.faceUpdated=Date.now();dirty=true;emitProfile();
  void flush().catch(e=>paketNotify(e.message));
}
function render(){emitProfile();}
if(typeof document!=='undefined'){
  window.addEventListener('paket-mode',((e:CustomEvent<string>)=>{playing=e.detail==='ride';previous=zero();lastTick=Date.now();if(playing&&initialized){local.stats.shifts++;dirty=true;}})as EventListener);
  window.addEventListener('paket-progress',((e:CustomEvent<Stats&{ended:boolean}>)=>{
    if(!initialized||!playing)return;const now=Date.now(),p=e.detail;
    if(!p.ended)local.stats.workMs+=Math.min(1000,Math.max(0,now-lastTick));lastTick=now;
    for(const k of ['delivered','photos','score','rings','markedVans','special']as const){const n=Number(p[k]);if(Number.isFinite(n)&&n>=0){local.stats[k]+=Math.max(0,n-previous[k]);previous[k]=n;}}
    dirty=true;
  })as EventListener);
  window.addEventListener('paket-day',()=>{previous=zero();lastTick=Date.now();if(playing&&initialized){local.stats.shifts++;dirty=true;}});
  document.addEventListener('visibilitychange',()=>{if(document.hidden)void flush().catch(()=>undefined);});
}
