declare global { var paketFrame: (milliseconds: number) => void; }

const canvas=document.querySelector<HTMLCanvasElement>('#game')!;
const area=canvas.parentElement!;
const scales=[1,0.85,0.7,0.55,0.43,0.34,0.26,0.2];
let software=false;
export function setSoftwareRendering(fallback:boolean){software=fallback;}
const storageKey=()=>software?'paket.render-quality.software.v2':'paket.render-quality.v2';
let tier=0, active=false, ready=false, viewport=0, warmup=0, frames:number[]=[];
let sampled=0, slowWindows=0, fastWindows=0, recoveryWindows=8;
let observedDensity: number | undefined;

function reset(warmupMs=900){
  warmup=warmupMs;
  frames=[];
  sampled=0;
  slowWindows=0;
  fastWindows=0;
}

function apply(){
  const scale=scales[tier];
  // Winit sizes the surface from the browser's device-pixel observer. Its
  // density can differ from devicePixelRatio, including with Chromium emulation.
  const layoutScale=active ? scale*devicePixelRatio/(observedDensity ?? devicePixelRatio) : 1;
  canvas.classList.toggle('bounded-scene',active && Math.abs(layoutScale-1)>0.001);
  canvas.dataset.renderScale=String(active ? scale : 1);
  canvas.style.setProperty('--scene-size', `${layoutScale*100}%`);
  canvas.style.setProperty('--scene-upscale', String(1/layoutScale));
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
  const softwareTier=scales.findIndex(scale=>pixels*scale**2<=220_000);
  tier=software ? (softwareTier===-1?scales.length-1:softwareTier) : 0;
  recoveryWindows=8;
  try{
    const saved=JSON.parse(localStorage.getItem(storageKey())??'null');
    if(saved && saved.pixels/pixels>0.75 && saved.pixels/pixels<1.35 && scales[saved.tier])tier=saved.tier;
  }catch{}
  reset();
  apply();
}

const observer=new ResizeObserver(([entry])=>{
  const {width}=entry.contentRect;
  const physical=entry.devicePixelContentBoxSize?.[0]?.inlineSize;
  observedDensity=width>0 && physical ? physical/width : undefined;
  selectViewport();
});
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
  // Use real game frames. A single tile rebuild or task switch must not set quality.
  frames.push(ms);
  sampled+=ms;
  if(sampled<1000 || frames.length<6)return;
  frames.sort((a,b)=>a-b);
  const duration=frames[Math.floor(frames.length*0.75)];
  frames=[];
  sampled=0;
  slowWindows=duration>27 ? slowWindows+1 : 0;
  fastWindows=duration<19 ? fastWindows+1 : 0;
  if(slowWindows>=2 && tier<scales.length-1){
    tier=Math.min(scales.length-1,tier+(duration>55?2:1));
    recoveryWindows=Math.min(recoveryWindows*2,32);
    reset();
    apply();
  }else if(fastWindows>=recoveryWindows && tier>0){
    tier--;
    reset();
    apply();
  }else if(slowWindows===0 || tier===scales.length-1){
    try{
      const calibration=JSON.stringify({pixels:viewport,tier});
      if(localStorage.getItem(storageKey())!==calibration)localStorage.setItem(storageKey(),calibration);
    }catch{}
  }
};

export {};
