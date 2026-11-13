const guide=document.querySelector<HTMLDialogElement>('#manual-dialog')!;
const canvas=document.querySelector<HTMLCanvasElement>('#game')!;
let riding=false;
window.addEventListener('paket-mode',((event:CustomEvent<string>)=>{riding=event.detail==='ride';if(guide.open)guide.close();})as EventListener);
function openGuide(){
  document.querySelector<HTMLDialogElement>('#settings-dialog')?.close();
  if(riding)globalThis.paketCommands.push({action:'menu',open:true});
  void document.exitPointerLock();guide.showModal();
}
for(const id of ['how-to-play','settings-guide'])document.getElementById(id)!.addEventListener('click',openGuide);
guide.querySelectorAll('[data-close-guide]').forEach(button=>button.addEventListener('click',()=>guide.close()));
guide.addEventListener('close',()=>{if(riding){globalThis.paketCommands.push({action:'menu',open:false});canvas.focus({preventScroll:true});}});
export {};
