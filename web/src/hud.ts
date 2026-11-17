import './hud.css';
import {deliveryGuidance, type DeliveryStop} from './guidance';
type Stop = DeliveryStop;
type IntercomOption = {kind:string; choice?:number; label:string; pressed?:boolean};
type Intercom = {revision:number; message:string; status:string; options:IntercomOption[]};
type Dispatcher = {message:string; timer:string; action:string};
type Hud = { selected:number; active:number; routes:Stop[]; score:number; delivered:number; day:number; minute:number; ready:boolean; ended:boolean; form:string; speed:number; difficulty:number; canChangeDifficulty:boolean; status:string; interaction:boolean; photo:boolean; photoReady:boolean; loading:boolean; bulk:boolean|null; vanDistance:number|null; stunSeconds:number; navigation?:{bearing:number;distance:number}|null; dispatcher?:Dispatcher|null; intercom?:Intercom|null };
const root=document.querySelector<HTMLElement>('#ride-hud')!;
const dialog=document.querySelector<HTMLDialogElement>('#settings-dialog')!;
const routeDirection=document.querySelector<HTMLElement>('#route-direction')!;
const routeArrow=routeDirection.querySelector('svg')!;
let arrowBearing=0;
const settings=document.querySelector<HTMLButtonElement>('#settings')!;
const canvas=document.querySelector<HTMLCanvasElement>('#game')!;
const send=(command:{action:string;[key:string]:unknown})=>globalThis.paketCommands.push(command);
const el=(id:string)=>document.getElementById(id)!;
let intercomRevision: number|undefined;
function showIntercom(panel?: Intercom|null) {
  el('intercom-panel').hidden = !panel;
  if (!panel) {
    el('intercom-options').replaceChildren();
    intercomRevision = undefined;
    return;
  }
  put('intercom-message', panel.message);
  put('intercom-status', panel.status);
  if (intercomRevision === panel.revision) return;
  const focused = (document.activeElement as HTMLElement|null)?.dataset.intercom;
  intercomRevision = panel.revision;
  const options = panel.options.map(({kind, choice, label, pressed}) => {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = label;
    button.dataset.intercom = kind + String(choice ?? '');
    if (pressed !== undefined) button.setAttribute('aria-pressed', String(pressed));
    button.onclick = () => {
      send({action:'intercom', revision:panel.revision, kind, choice});
      if (kind === 'close' || kind === 'reply') canvas.focus({preventScroll:true});
    };
    return button;
  });
  el('intercom-options').replaceChildren(...options);
  options.find(b => b.dataset.intercom === focused)?.focus({preventScroll:true});
}
const put=(id:string,value:string)=>{const node=el(id);if(node.textContent!==value)node.textContent=value;};
const states:Record<Stop['state'],string>={'in-van':'In the van',carrying:'In your hands','in-flight':'In flight','set-aside':'Set aside','photo-due':'Photo needed',filed:'Delivered',handed:'Handed over'};
const buttons=new Map<number,HTMLButtonElement>();
let ride=false, state:Hud|undefined, lastStatus='', expanded=false;
let sensitivity=0.0024;
try {const value=Number(localStorage.getItem('paket.sensitivity'));if(value>=0.0008&&value<=0.006)sensitivity=value;}catch{}
const slider=el('sensitivity') as HTMLInputElement;slider.value=String(sensitivity);
const meters=(value:number)=>value>=1000?`${(value/1000).toFixed(1)} km`:`${Math.round(value)} m`;
function select(index:number){send({action:'delivery',index});canvas.focus({preventScroll:true});}
function showStops(open:boolean){expanded=open;el('route-list').hidden=!open;el('route-toggle').setAttribute('aria-expanded',String(open));}
el('route-toggle').onclick=()=>showStops(!expanded);
function openSettings(){
  if(!ride||state?.interaction||document.querySelector('dialog[open]'))return;
  send({action:'menu',open:true});void document.exitPointerLock();dialog.showModal();
}
function closeSettings(){
  dialog.close();send({action:'menu',open:false});canvas.focus({preventScroll:true});
}
settings.onclick=openSettings;
el('settings-close').onclick=closeSettings;
el('resume-game').onclick=closeSettings;
el('change-location').onclick=()=>{closeSettings();send({action:'change-location'});};
dialog.addEventListener('cancel',e=>{e.preventDefault();closeSettings();});
slider.oninput=()=>{
  sensitivity=Number(slider.value);send({action:'sensitivity',value:sensitivity});
  try{localStorage.setItem('paket.sensitivity',String(sensitivity));}catch{}
};
el('difficulty').onchange=e=>send({action:'difficulty',value:Number((e.target as HTMLSelectElement).value)});
window.addEventListener('keydown',e=>{
  if(!ride)return;
  // Winit otherwise prevents the browser default for keys reaching the canvas.
  // Keep system/browser tab-switching shortcuts outside the game's input path.
  if(e.key==='Tab'&&(e.metaKey||e.ctrlKey||e.altKey)){e.stopImmediatePropagation();return;}
  if(e.code==='KeyR'&&!e.metaKey&&!e.ctrlKey&&!e.altKey&&!state?.interaction&&!document.querySelector('dialog[open]')
    &&!(e.target instanceof HTMLElement&&(e.target.isContentEditable||e.target.matches('input, textarea, select')))){
    e.preventDefault();e.stopImmediatePropagation();
    if(!e.repeat)send({action:'open-intercom'});
    return;
  }
  if(e.key==='Tab'&&!e.metaKey&&!e.ctrlKey&&!e.altKey&&!document.querySelector('dialog[open]')&&!state?.interaction
    &&(document.activeElement===canvas||document.pointerLockElement===canvas)){
    e.preventDefault();e.stopImmediatePropagation();
    if(!e.repeat)send({action:'delivery-step',delta:e.shiftKey?-1:1});
    return;
  }
  if(e.key==='Escape'&&state?.intercom){
    e.preventDefault();e.stopImmediatePropagation();
    if(!e.repeat)send({action:'intercom',revision:state.intercom.revision,kind:'close'});
    canvas.focus({preventScroll:true});
    return;
  }
  if(e.key==='F1'){e.preventDefault();e.stopImmediatePropagation();if(dialog.open)closeSettings();else openSettings();}
  else if(document.querySelector('dialog[open]'))e.stopImmediatePropagation();
},true);
window.addEventListener('paket-mode',((e:CustomEvent<string>)=>{
  ride=e.detail==='ride';root.hidden=!ride;settings.hidden=!ride;showStops(false);lastStatus='';buttons.clear();el('route-list').replaceChildren();
  if(dialog.open)dialog.close();
  showIntercom();el('dispatcher').hidden=true;routeDirection.hidden=true;
  if(ride)send({action:'sensitivity',value:sensitivity});
}) as EventListener);
window.addEventListener('paket-hud',((event:CustomEvent<Hud>)=>{
  const s=state=event.detail;root.classList.toggle('interacting',s.interaction);
  put('shift-time',s.ready?`${String(Math.floor(s.minute/60)).padStart(2,'0')}:${String(s.minute%60).padStart(2,'0')}`:'—:—');
  put('shift-label',s.ended?`DAY ${s.day} COMPLETE`:s.ready?`DAY ${s.day} · ENDS 17:00`:'JOINING SHIFT');
  el('shift-progress').style.width=`${Math.max(0,Math.min(100,(s.minute-540)/480*100))}%`;
  put('hud-score',String(s.score));put('route-total',`${s.delivered} / ${s.routes.length}`);
  put('speed-value',String(Math.round(s.speed)).padStart(2,'0'));el('speedometer').hidden=s.form!=='van';
  el('delivery-card').hidden=s.form==='dog';el('hud-loading').hidden=!s.loading;
  const nav=s.navigation;
  const showArrow=s.ready&&!s.loading&&!s.ended&&!s.interaction&&!s.photo&&nav&&Number.isFinite(nav.bearing);
  if(showArrow){
    if(routeDirection.hidden){
      arrowBearing=nav.bearing;
    }else{
      // Keep the rotation continuous when the bearing wraps at ±180°.
      const delta=(nav.bearing-arrowBearing)%360;
      arrowBearing+=(delta+540)%360-180;
    }
    routeArrow.style.rotate=`${arrowBearing}deg`;
    const distance=meters(nav.distance);
    const accessible=`Direction to destination · ${distance}`;
    if(routeDirection.getAttribute('aria-label')!==accessible)routeDirection.setAttribute('aria-label',accessible);
    put('route-direction-distance',distance);
  }
  routeDirection.hidden=!showArrow;
  const selected=s.routes[s.selected],active=s.routes[s.active];
  if(selected){
    put('stop-number',`STOP ${String(s.selected+1).padStart(2,'0')} / ${s.routes.length}`);
    put('stop-address',selected.address);put('stop-recipient',selected.name);put('stop-distance',meters(selected.state==='set-aside'?(selected.parcelDistance??selected.distance):selected.distance));
    put('stop-state',states[selected.state]);
  }
  const holding=active&&['carrying','in-flight','photo-due'].includes(active.state)&&s.active!==s.selected;
  el('held-parcel').hidden=!holding;
  if(holding)put('held-parcel',`${active.state==='carrying'?'Still in your hands':active.state==='in-flight'?'In flight':'Awaiting photo'}: ${active.address}`);
  for(const stop of s.routes){
    let button=buttons.get(stop.index);
    if(!button){
      button=document.createElement('button');button.className='route-stop';button.type='button';
      button.innerHTML='<span class="route-index"></span><span class="route-copy"><b></b><small></small><em></em></span><span class="route-distance"></span>';
      button.onclick=()=>select(stop.index);buttons.set(stop.index,button);el('route-list').append(button);
    }
    button.setAttribute('aria-pressed',String(stop.index===s.selected));
    button.querySelector('.route-index')!.textContent=String(stop.index+1).padStart(2,'0');
    button.querySelector('b')!.textContent=stop.address;button.querySelector('small')!.textContent=stop.name;
    button.querySelector('em')!.textContent=states[stop.state];button.querySelector('.route-distance')!.textContent=meters(stop.state==='set-aside'?(stop.parcelDistance??stop.distance):stop.distance);
    button.disabled=s.interaction;
  }
  const guidance=deliveryGuidance(s);
  put('delivery-step',guidance.step);put('delivery-detail',guidance.detail);
  const dispatch = s.form === 'courier' ? s.dispatcher : null;
  el('dispatcher').hidden = !dispatch;
  if(dispatch){
    put('dispatcher-message',dispatch.message);
    put('dispatcher-timer',dispatch.timer);
  }
  showIntercom(s.intercom);
  put('hud-action',dispatch && s.active===s.selected && s.stunSeconds <= 0 && !s.ended ? dispatch.action : guidance.action);el('hud-action').classList.toggle('photo-ready',s.photoReady);
  el('shock-status').hidden=!(s.stunSeconds>0);
  if(s.stunSeconds>0)put('shock-status',`BITTEN · IN SHOCK · ${s.stunSeconds.toFixed(1)}s`);
  const difficulty=el('difficulty') as HTMLSelectElement;
  if(document.activeElement!==difficulty)difficulty.value=String(s.difficulty);
  difficulty.disabled=!s.canChangeDifficulty;
  put('difficulty-note',s.canChangeDifficulty?'Choose before the first doorbell.':'Set by the host; locked after the first doorbell.');
  if(s.status&&s.status!==lastStatus){lastStatus=s.status;globalThis.paketNotify(s.status);}
}) as EventListener);
export {};
