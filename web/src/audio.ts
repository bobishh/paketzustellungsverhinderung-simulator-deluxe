import {StepDistance,streetLoops,type Effect,type SamplePlayer,type StreetState} from './street-sound';

const toggle=document.querySelector<HTMLInputElement>('#game-sound')!;
try{toggle.checked=localStorage.getItem('paket.sound')!=='off';}catch{}
let player:SamplePlayer|undefined,ride=false,state:StreetState|undefined;
const steps=new StepDistance();
function refresh(effects:Effect[]=[]){
 const active=ride&&Boolean(state?.active)&&!document.hidden&&!document.querySelector('dialog[open]');
 player?.sync({active,volume:toggle.checked?50:0,effects,loops:active&&state?streetLoops(state):[]});
 if(!active)steps.reset();
}
toggle.addEventListener('change',()=>{
 try{localStorage.setItem('paket.sound',toggle.checked?'on':'off');}catch{}
 refresh();
});
const audioModule='/assets/job-sample-audio.js';
void import(/* @vite-ignore */audioModule).then(module=>{
 player=module.createSampleAudio({target:document});refresh();
}).catch(()=>{ /* Audio downloads never prevent a shift. */ });
window.addEventListener('paket-mode',event=>{
 ride=(event as CustomEvent<string>).detail==='ride';state=undefined;routes=new Map();photo=false;steps.reset();refresh();
});
window.addEventListener('street-audio',event=>{
 state=(event as CustomEvent<StreetState>).detail;
 const effects:Effect[]=[];
 if(state.active&&ride&&state.form!=='van'&&steps.advance(state.position,Math.abs(state.verticalSpeed)>1.5,state.form==='dog'))effects.push('step-pavement');
 if(state.form==='van')steps.reset();
 refresh(effects);
});
window.addEventListener('game-sound',event=>refresh([(event as CustomEvent<string>).detail]));
window.addEventListener('paket-bite',()=>refresh(['bite']));
let routes=new Map<number,string>(),photo=false;
window.addEventListener('paket-hud',event=>{
 const hud=(event as CustomEvent<{routes:{index:number;state:string}[];photoReady:boolean}>).detail;
 const effects:Effect[]=[];
 for(const route of hud.routes){
  const previous=routes.get(route.index);
  if(previous&&previous!==route.state){
   if(route.state==='carrying')effects.push('parcel');
   else if(route.state==='in-flight')effects.push('cloth');
   else if(['set-aside','photo-due'].includes(route.state))effects.push('soft-hit');
   else if(['filed','handed'].includes(route.state))effects.push('success');
  }
 }
 routes=new Map(hud.routes.map(route=>[route.index,route.state]));
 if(hud.photoReady&&!photo)effects.push('camera');photo=hud.photoReady;
 refresh(effects);
});
document.addEventListener('visibilitychange',()=>refresh());
new MutationObserver(()=>refresh()).observe(document.body,{subtree:true,attributes:true,attributeFilter:['open']});
