type Vote={day:number;ended:boolean;remaining:number;choice:number|null;options:string[];players:{name:string;score:number;delivered:number;vote:number|null}[]};
const dialog=document.querySelector<HTMLDialogElement>('#shift-vote-dialog')!;
const options=document.querySelector('#vote-options')!;
let riding=false;
const send=(command:{action:string;[key:string]:unknown})=>globalThis.paketCommands.push(command);
window.addEventListener('paket-mode',((e:CustomEvent<string>)=>{riding=e.detail==='ride';if(dialog.open)dialog.close();})as EventListener);
window.addEventListener('paket-day',()=>{if(dialog.open)dialog.close();send({action:'menu',open:false});document.querySelector<HTMLCanvasElement>('#game')!.focus();});
dialog.addEventListener('cancel',e=>e.preventDefault());
window.addEventListener('paket-hud',((event:CustomEvent<{vote:Vote}>)=>{
  const v=event.detail.vote;if(!riding||!v?.ended||v.options.length!==3)return;
  if(!dialog.open){document.querySelectorAll<HTMLDialogElement>('dialog[open]').forEach(d=>d.close());send({action:'menu',open:true});globalThis.paketWantsPointer=false;void document.exitPointerLock();dialog.showModal();}
  document.querySelector('#vote-title')!.textContent=`Day ${v.day} complete`;
  const rows=v.players.slice().sort((a,b)=>b.score-a.score).map(p=>{const row=document.createElement('tr');for(const text of [p.name,String(p.delivered),String(p.score)]){const cell=document.createElement('td');cell.textContent=text;row.append(cell);}return row;});
  document.querySelector('#vote-scores')!.replaceChildren(...rows);
  if(options.childElementCount!==3){options.replaceChildren();for(let i=0;i<3;i++){const b=document.createElement('button');b.type='button';b.addEventListener('click',()=>send({action:'shift-vote',choice:i}));options.append(b);}}
  Array.from(options.children).forEach((node,i)=>{const b=node as HTMLButtonElement;const votes=v.players.filter(p=>p.vote===i).length;b.textContent=`${v.options[i]} · ${votes} ${votes===1?'vote':'votes'}`;b.setAttribute('aria-pressed',String(v.choice===i));});
  document.querySelector('#vote-countdown')!.textContent=`Next day in ${v.remaining}s${v.choice!==null?' · Vote recorded':''}`;
})as EventListener);
export {};
