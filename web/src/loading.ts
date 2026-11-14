const overlay=document.querySelector<HTMLElement>('#world-loading')!;
const label=document.querySelector<HTMLElement>('#world-loading-text')!;
const spinner=overlay.querySelector<HTMLElement>('.spinner')!;
const retry=document.querySelector<HTMLButtonElement>('#retry-map')!;
let mode='';
function show(pending:boolean,text:string,failed=false){
  overlay.hidden=!pending&&!failed;
  overlay.setAttribute('aria-busy',String(pending));
  spinner.hidden=!pending;label.textContent=text;retry.hidden=!failed;
}
export function showStartup(){if(!mode)show(true,'Starting renderer…');}
window.addEventListener('paket-mode',((e:CustomEvent<string>)=>{
  mode=e.detail;overlay.classList.toggle('map-loading',mode==='map');
  show(true,mode==='map'?'Loading map…':'Starting neighbourhood…');
})as EventListener);
window.addEventListener('paket-map-loading',((e:CustomEvent<{ready:boolean;pending:boolean;failed:boolean}>)=>{
  if(mode!=='map')return;
  const s=e.detail;
  show(s.pending,s.failed?'Some map tiles could not load.':s.ready?'Loading map tiles…':'Loading map…',s.failed);
})as EventListener);
window.addEventListener('paket-hud',((e:CustomEvent<{ready:boolean}>)=>{
  if(mode==='ride')show(!e.detail.ready,'Starting neighbourhood…');
})as EventListener);
retry.onclick=()=>{globalThis.paketCommands.push({action:'retry-map'});show(true,'Loading map…');};
export {};
