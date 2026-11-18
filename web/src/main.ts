import './style.css';
import './mouse';
import './audio';
import {setSoftwareRendering} from './render-quality';
import {initGameAnalytics} from './analytics';
import {initProfile} from './profile';
import {restoreIdentity} from './identity';
import {mesh} from './mesh';
import {initServices,publish,setPhotoDownload} from './services';
declare global {var paketArgs:string[];var paketMesh:typeof mesh;var paketPhoto:(bytes:Uint8Array,index:number)=>void;}
const callbackHash=location.pathname==='/identity/return'?location.hash:null;
if(callbackHash!==null)history.replaceState(null,'','/identity/return');
globalThis.paketNotify=text=>publish('paket-notice',text);
initGameAnalytics();
const ready=restoreIdentity(callbackHash).then(async path=>{if(path?.startsWith('/')&&!path.startsWith('//'))history.replaceState(null,'',path);await initProfile();}).catch(error=>{paketNotify(`Identity: ${error.message}`);throw error;});
void ready.catch(()=>undefined);
initServices(ready);
let photoUrl:string|undefined,photoName='';
globalThis.paketPhoto=(bytes,index)=>{if(photoUrl)URL.revokeObjectURL(photoUrl);photoUrl=URL.createObjectURL(new Blob([bytes.slice().buffer],{type:'image/png'}));photoName=`delivery-${index+1}-${Date.now()}.png`;publish('paket-photo',{ready:true});paketNotify('Photo filed. Open Photo to download.');};
setPhotoDownload(()=>{if(!photoUrl){paketNotify('Take a delivery photo first.');return;}const link=document.createElement('a');link.href=photoUrl;link.download=photoName;link.click();});
const boot=document.querySelector<HTMLElement>('#bootstrap')!,bootStatus=document.querySelector<HTMLElement>('#boot-status')!,retry=document.querySelector<HTMLButtonElement>('#boot-retry')!;
let bootTimer:ReturnType<typeof setTimeout>|undefined;
window.addEventListener('paket-bootstrap-error',event=>{clearTimeout(bootTimer);boot.hidden=false;bootStatus.textContent=String((event as CustomEvent).detail);retry.hidden=false;});
window.addEventListener('paket-ui-ready',()=>{clearTimeout(bootTimer);boot.hidden=true;document.querySelector<HTMLCanvasElement>('#game')!.focus({preventScroll:true});});
retry.onclick=()=>location.reload();
async function start(){
 bootTimer=setTimeout(()=>{bootStatus.textContent='Loading is taking longer than expected. Retry if it does not finish.';retry.hidden=false;},90000);
 try{
  const gpu=(navigator as Navigator&{gpu?:{requestAdapter():Promise<{info?:{isFallbackAdapter?:boolean}}|null>}}).gpu;
  const adapter=await gpu?.requestAdapter();if(!adapter)throw new Error('WebGPU is unavailable. Enable hardware graphics or use a supported browser.');
  setSoftwareRendering(Boolean(adapter.info?.isFallbackAdapter));await ready;
  const query=new URLSearchParams(location.search),join=new URLSearchParams(location.hash.slice(1)).has('join');
  const lat=Number(query.get('lat')),lon=Number(query.get('lon'));
  globalThis.paketArgs=!join&&query.has('lat')&&query.has('lon')&&Number.isFinite(lat)&&Math.abs(lat)<=85&&Number.isFinite(lon)&&Math.abs(lon)<=180?['--lat',String(lat),'--lon',String(lon)]:[];
  if(query.has('press-shot')){paketArgs.push('--press-shot');document.body.classList.add('press-shot');}
  if(query.has('inspect-courier'))paketArgs.push('--inspect-courier');
  const url='/game/paket_game.js';const{default:init}=await import(/* @vite-ignore */url);await init();document.querySelector<HTMLCanvasElement>('#game')!.focus();
 }catch(error){clearTimeout(bootTimer);boot.hidden=false;bootStatus.textContent=error instanceof Error?error.message:String(error);retry.hidden=false;}
}
void start();
