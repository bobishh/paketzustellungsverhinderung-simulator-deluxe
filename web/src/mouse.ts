/** Pointer-lock and OS keyboard capabilities; Bevy owns visible controls and text. */
const canvas=document.querySelector<HTMLCanvasElement>('#game')!;
type TextField='address'|'invite';
type Region={x:number;y:number;width:number;height:number;capture:boolean;field?:TextField|null;value?:string|null;selection?:[number,number]|null;readOnly?:boolean};
type TextInput={field:TextField;value:string;start:number;end:number;submit?:boolean};
declare global{var paketCapture:(()=>void)|undefined;var paketUiHitRects:Region[]|undefined;var paketTextInputs:TextInput[];}
globalThis.paketTextInputs=[];
let riding=false,blocked=false,pending=false,captureAllowed=false;
let imePointer:{id:number;region:Region}|undefined,editing:TextField|undefined;
const ime=document.createElement('input');
ime.autocomplete='off';ime.setAttribute('aria-hidden','true');ime.tabIndex=-1;
ime.style.cssText='position:fixed;left:-10000px;top:0;width:1px;height:1px;opacity:0';
document.body.append(ime);
const owns=()=>document.pointerLockElement===canvas;
const hitAt=(event:PointerEvent)=>{const bounds=canvas.getBoundingClientRect(),x=event.clientX-bounds.left,y=event.clientY-bounds.top;return globalThis.paketUiHitRects?.find(r=>x>=r.x&&y>=r.y&&x<r.x+r.width&&y<r.y+r.height);};
function cancelIme(){imePointer=undefined;editing=undefined;ime.blur();}
function publishText(submit=false){if(!editing)return;const change:TextInput={field:editing,value:ime.value,start:ime.selectionStart??ime.value.length,end:ime.selectionEnd??ime.value.length,...(submit?{submit:true}:{})};if(paketTextInputs.length>=32)paketTextInputs.shift();paketTextInputs.push(change);}
ime.addEventListener('input',()=>publishText());
ime.addEventListener('select',()=>publishText());
ime.addEventListener('blur',()=>{editing=undefined;});
ime.addEventListener('keydown',event=>{if(event.isComposing)return;if(event.key==='Enter'){event.preventDefault();publishText(true);cancelIme();canvas.focus({preventScroll:true});}else if(event.key==='Escape'){event.preventDefault();cancelIme();canvas.focus({preventScroll:true});}});
function failed(){if(!pending)return;pending=false;paketNotify('Mouse capture was blocked. Click Capture mouse to retry.');}
function capture(){if(!riding||blocked||!captureAllowed||pending||owns())return;canvas.focus({preventScroll:true});pending=true;try{void canvas.requestPointerLock()?.catch(failed);}catch{failed();}}
globalThis.paketCapture=capture;
canvas.addEventListener('pointerdown',event=>{
 const hit=hitAt(event);
 if(hit?.field&&!hit.readOnly){imePointer={id:event.pointerId,region:hit};return;}
 cancelIme();if(!hit||hit.capture)capture();
});
// Winit focuses its canvas on pointerdown. Focus the hidden OS input only on the
// matching trusted pointerup, after that handler, while activation is available.
canvas.addEventListener('pointerup',event=>{
 const pending=imePointer;if(!pending||pending.id!==event.pointerId)return;imePointer=undefined;
 const hit=hitAt(event);if(!hit?.field||hit.field!==pending.region.field||hit.readOnly)return;
 const region=globalThis.paketUiHitRects?.find(r=>r.field===hit.field&&!r.readOnly)??hit;
 editing=region.field??undefined;if(!editing)return;
 ime.type=editing==='address'?'search':'url';ime.inputMode=editing==='address'?'search':'url';ime.maxLength=editing==='address'?180:8192;
 ime.value=region.value??'';ime.focus({preventScroll:true});
 const selection=region.selection??[ime.value.length,ime.value.length];try{ime.setSelectionRange(selection[0],selection[1]);}catch{/* Keyboard types may not expose a selection on every browser. */}
});
canvas.addEventListener('pointercancel',event=>{if(imePointer?.id===event.pointerId)cancelIme();});
window.addEventListener('blur',cancelIme);
document.addEventListener('visibilitychange',()=>{if(document.hidden)cancelIme();});
document.addEventListener('pointerlockerror',failed);
document.addEventListener('pointerlockchange',()=>{pending=false;if(owns()&&(!riding||blocked)){document.exitPointerLock();return;}if(riding)paketCommands.push({action:'look',active:owns()});});
window.addEventListener('paket-mode',((e:CustomEvent<string>)=>{cancelIme();riding=e.detail==='ride';blocked=false;if(!riding&&owns())document.exitPointerLock();})as EventListener);
window.addEventListener('paket-hud',((e:CustomEvent<{interaction?:boolean;menu?:boolean}>)=>{blocked=Boolean(e.detail.interaction||e.detail.menu);if(blocked&&owns())document.exitPointerLock();})as EventListener);
window.addEventListener('paket-ui-regions',((e:CustomEvent<{captureAllowed:boolean;regions:Region[];panel?:string}>)=>{captureAllowed=e.detail.captureAllowed;globalThis.paketUiHitRects=e.detail.regions;if(e.detail.panel)cancelIme();if(!captureAllowed&&owns())document.exitPointerLock();})as EventListener);
export {};
