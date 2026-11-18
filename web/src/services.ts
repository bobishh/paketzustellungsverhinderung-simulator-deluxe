/** Browser capabilities only. Bevy owns every game control and panel. */
import {savedInvitation, mesh} from './mesh';
import {availableMeshes} from './mesh-directory';
import {requestNeighbourhood} from './geolocation';
import {emitProfile, newFace} from './profile';
import {signIn} from './identity';
export type Command={action:string;[key:string]:unknown};
declare global {var paketCommands:Command[];var paketPublicMesh:boolean;var paketNotify:(text:string)=>void;}
globalThis.paketCommands=[];
globalThis.paketPublicMesh=false;
export const send=(command:Command)=>paketCommands.push(command);
export const publish=(name:string,data:unknown)=>send({action:'ui-event',name,data});
let mode='',invitation='',locationGeneration=0,searchGeneration=0,refreshing=false,searching=false;
const message=(error:unknown)=>error instanceof Error?error.message:String(error);
const status=(status:string,extra:Record<string,unknown>={})=>publish('paket-services',{status,...extra});
let download:()=>void=()=>paketNotify('Take a delivery photo first.');
export function setPhotoDownload(action:()=>void){download=action;}
export function initServices(ready:Promise<unknown>){
 void ready.then(()=>{
   publish('paket-settings',{sound:localStorage.getItem('paket.sound')!=='off',sensitivity:Number(localStorage.getItem('paket.sensitivity'))||0.0024});
   emitProfile();
 }).catch(()=>undefined);
 window.addEventListener('paket-service',((event:CustomEvent<Record<string,unknown>>)=>{
   const request=event.detail;
   void (async()=>{
    switch(request.kind){
     case 'profile':await ready;emitProfile();break;
     case 'new-face':await ready;newFace();break;
     case 'signin':await ready;await signIn();break;
     case 'fullscreen':await(document.fullscreenElement?document.exitFullscreen():document.documentElement.requestFullscreen());break;
     case 'release':if(document.pointerLockElement)document.exitPointerLock();break;
     case 'capture':break; // Trusted canvas pointerdown owns this permission request.
     case 'download-photo':download();break;
     case 'public':globalThis.paketPublicMesh=Boolean(request.listed);status(paketPublicMesh?'Mesh will be listed publicly.':'Invitation-only mesh.');break;
     case 'sound':localStorage.setItem('paket.sound',request.enabled?'on':'off');window.dispatchEvent(new CustomEvent('paket-sound-setting',{detail:Boolean(request.enabled)}));publish('paket-settings',{sound:Boolean(request.enabled)});break;
     case 'sensitivity':localStorage.setItem('paket.sensitivity',String(request.value));break;
     case 'resume':{const link=savedInvitation();if(!link)throw new Error('No saved mesh. Create one or paste an invitation.');send({action:'join',link});break;}
     case 'selection':locationGeneration++;searchGeneration++;status('',{locationPending:false,searchPending:false,places:[]});break;
     case 'refresh':{
       if(refreshing)return;refreshing=true;status('Looking for public meshes…',{directoryPending:true});
       try{const meshes=await availableMeshes();status(meshes.length?'Choose a public mesh.':'No public meshes yet.',{meshes:meshes.map(v=>({link:JSON.stringify(v),lat:v.origin.lat,lon:v.origin.lon,name:v.room.slice(0,6)}))});}
       finally{refreshing=false;publish('paket-services',{directoryPending:false});}break;
     }
     case 'location':{
       const generation=++locationGeneration;status('Allow location to choose your rounded neighbourhood.',{locationPending:true});
       try{const point=await requestNeighbourhood(navigator.geolocation);if(generation!==locationGeneration||mode!=='map')return;send({action:'select',...point});status('Neighbourhood selected. Drag the map to fine-tune.');}
       finally{if(generation===locationGeneration)publish('paket-services',{locationPending:false});}break;
     }
     case 'search':{
       if(searching)return;const query=String(request.query??'').trim();if(query.length<3){status('Type at least three characters.');return;}
       searching=true;const generation=++searchGeneration;status('Finding that address…',{searchPending:true,places:[]});
       try{const response=await fetch(`/api/address?q=${encodeURIComponent(query.slice(0,180))}`,{signal:AbortSignal.timeout(15000)});
       let data:unknown;
       if(response.ok)data=await response.json();else{
        const fallback=await fetch(`https://photon.komoot.io/api/?q=${encodeURIComponent(query.slice(0,180))}&limit=5`,{signal:AbortSignal.timeout(12000)});if(!fallback.ok)throw new Error('Address search unavailable. Retry or choose on the map.');
        const body=await fallback.json();data=(body.features??[]).filter((f:any)=>f.geometry?.type==='Point').map((f:any)=>({lat:f.geometry.coordinates[1],lon:f.geometry.coordinates[0],name:[f.properties.name,[f.properties.street,f.properties.housenumber].filter(Boolean).join(' '),f.properties.city||f.properties.district,f.properties.country].filter(Boolean).join(', ')}));
       }if(generation!==searchGeneration||mode!=='map')return;
       const places=Array.isArray(data)?data.filter((v:any)=>Number.isFinite(v.lat)&&Number.isFinite(v.lon)&&Math.abs(v.lat)<=85&&Math.abs(v.lon)<=180).slice(0,5):[];status(places.length?'Select a result.':'No results. Add a city or postal code.',{places});}
       finally{searching=false;publish('paket-services',{searchPending:false});}break;
     }
    }
   })().catch(error=>{const text=message(error);status(text);paketNotify(text);});
 })as EventListener);
 window.addEventListener('paket-mode',((event:CustomEvent<string>)=>{
  mode=event.detail;locationGeneration++;
  if(mode==='map'){
   const join=new URLSearchParams(location.hash.slice(1)).get('join');
   if(join){history.replaceState(null,'',location.pathname+location.search);void ready.then(()=>send({action:'join',link:`${location.origin}/#join=${join}`})).catch(e=>paketNotify(message(e)));}
  }
 })as EventListener);
 for(const name of ['paket-session','paket-members','paket-profile','paket-identity'])window.addEventListener(name,((e:CustomEvent<unknown>)=>{publish(name,e.detail);if(name==='paket-session')invitation=(e.detail as{link?:string}|null)?.link??'';})as EventListener);
 // The capability bridge never renders game widgets or takes over an existing tab.
 window.addEventListener('paket-service',((event:CustomEvent<{kind:string}>)=>{if(event.detail.kind==='copy-invite'&&invitation)void navigator.clipboard.writeText(invitation).catch(e=>paketNotify(message(e)));})as EventListener);
 globalThis.paketMesh=mesh;
}
