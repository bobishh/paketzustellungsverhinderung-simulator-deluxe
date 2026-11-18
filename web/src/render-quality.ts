declare global { var paketFrame: (milliseconds: number) => void; var paketSceneScale: number; }

const canvas=document.querySelector<HTMLCanvasElement>('#game')!;
const area=canvas.parentElement!;
const hardwareScales=[1,0.85,0.7];
const softwareScales=[...hardwareScales,0.55,0.43,0.34,0.26,0.2];
const scales=()=>software?softwareScales:hardwareScales;
let software=false;
export function setSoftwareRendering(fallback:boolean){software=fallback;}
const storageKey=()=>software?'paket.render-quality.software.v2':'paket.render-quality.v3';
let tier=0, active=false, ready=false, viewport=0, warmup=0, frames:number[]=[];
let sampled=0, slowWindows=0, fastWindows=0, retryAfter=0, rejectedDuration=0;
let trial: {tier: number; duration: number; windows: number} | undefined;

function reset(warmupMs=900){
  warmup=warmupMs;
  frames=[];
  sampled=0;
  slowWindows=0;
  fastWindows=0;
}

function save(){
  try{
    const calibration=JSON.stringify({pixels:viewport,tier});
    if(localStorage.getItem(storageKey())!==calibration)localStorage.setItem(storageKey(),calibration);
  }catch{}
}

function apply(){
  const scale=active ? scales()[tier] : 1;
  // The canvas and Bevy UI keep full device resolution. Rust resizes only the
  // street render texture, then composites it beneath the interface.
  globalThis.paketSceneScale=scale;
  canvas.dataset.renderScale=String(scale);
}

function selectViewport(){
  if(!active)return;
  const pixels=area.clientWidth*area.clientHeight*devicePixelRatio**2;
  if(!pixels)return;
  if(viewport && pixels/viewport>0.75 && pixels/viewport<1.35){
    apply();
    return;
  }
  viewport=pixels;
  const softwareTier=softwareScales.findIndex(scale=>pixels*scale**2<=220_000);
  tier=software ? (softwareTier===-1?softwareScales.length-1:softwareTier) : 0;
  trial=undefined;
  retryAfter=0;
  rejectedDuration=0;
  try{
    const saved=JSON.parse(localStorage.getItem(storageKey())??'null');
    if(saved && saved.pixels/pixels>0.75 && saved.pixels/pixels<1.35 && scales()[saved.tier])tier=saved.tier;
  }catch{}
  reset();
  apply();
}

const observer=new ResizeObserver(()=>selectViewport());
try{observer.observe(area,{box:'device-pixel-content-box'});}
catch{observer.observe(area);}
window.addEventListener('resize',selectViewport);
document.addEventListener('visibilitychange',()=>reset());
window.addEventListener('paket-mode',((event:CustomEvent<string>)=>{
  active=event.detail==='ride' && !document.body.classList.contains('press-shot');
  ready=false;
  viewport=0;
  selectViewport();
  apply();
})as EventListener);
window.addEventListener('paket-hud',((event:CustomEvent<{ready:boolean;loading?:boolean}>)=>{
  const loaded=event.detail.ready && !event.detail.loading;
  if(ready!==loaded){
    ready=loaded;
    reset();
  }
})as EventListener);

globalThis.paketFrame=ms=>{
  if(!active || !ready || document.hidden || !Number.isFinite(ms) || ms<=0)return;
  if(warmup>0){warmup-=ms;return;}
  retryAfter=Math.max(0,retryAfter-ms);
  // Ignore isolated tile rebuilds. Only keep a lower resolution if it speeds up rendering.
  frames.push(ms);
  sampled+=ms;
  if(sampled<1000 || frames.length<6)return;
  frames.sort((a,b)=>a-b);
  const duration=frames[Math.floor(frames.length*0.6)];
  frames=[];
  sampled=0;
  if(trial){
    if(++trial.windows<2)return;
    if(duration>trial.duration*0.9){
      tier=trial.tier;
      retryAfter=30000;
      rejectedDuration=trial.duration;
      reset();
      apply();
    }else{
      rejectedDuration=0;
      save();
    }
    trial=undefined;
    return;
  }
  slowWindows=duration>27 ? slowWindows+1 : 0;
  fastWindows=duration<19 ? fastWindows+1 : 0;
  const workloadChanged=Math.abs(duration-rejectedDuration)>rejectedDuration*0.2;
  if(slowWindows>=2 && (!retryAfter || workloadChanged) && tier<scales().length-1){
    trial={tier,duration,windows:0};
    tier++;
    reset();
    apply();
  }else if(fastWindows>=8 && tier>0){
    tier--;
    reset();
    apply();
  }else if(slowWindows===0){
    save();
  }
};

export {};
