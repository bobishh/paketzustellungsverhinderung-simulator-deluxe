// Shared recording player for Job Hunt, Paket and Feed the Poor. No synthesis.
import {sampleCues} from '/assets/job-audio-cues.js';

export function createSampleAudio({target=document,storageKey,initialVolume=50,recordings={}}={}) {
  let context,master,volume=initialVolume;
  const bankUrl='/assets/job-audio-bank.mp3';
  const buffers=new Map(),loading=new Set();
  const recordingUrls=new Set(Object.values(recordings).flatMap(cues=>cues.map(cue=>cue.url)));
  let desired={active:false,effects:[],loops:[]};
  const voices=new Set(),loops=new Map(),variants=new Map();
  const clamp=(value,max=1)=>Math.max(0,Math.min(max,Number(value)||0));
  const stopVoice=voice=>{
    voices.delete(voice);
    voice.source.onended=()=>{voice.source.disconnect();voice.gain.disconnect();voice.pan?.disconnect();};
    try {
      voice.gain.gain.setTargetAtTime(0,context.currentTime,.015);
      voice.source.stop(context.currentTime+.06);
    } catch {voice.source.onended();}
  };
  const stop=()=>{for(const voice of [...voices])stopVoice(voice);loops.clear();};
  const audible=()=>desired.active&&!document.hidden&&volume>0&&context?.state==='running';
  const choose=(name,variant)=>{
    const list=recordings[name]??sampleCues[name];if(!list)return null;
    const index=variants.get(name)||0;variants.set(name,index+1);
    return list[(Number.isInteger(variant)?Math.abs(variant):index)%list.length];
  };
  const start=(cue,settings={})=>{
    const buffer=buffers.get(cue.url??bankUrl);
    if(!buffer)return;
    const source=context.createBufferSource(),gain=context.createGain();
    const pan=typeof context.createStereoPanner==='function'?context.createStereoPanner():null;
    source.buffer=buffer;
    source.playbackRate.value=clamp(settings.rate||1,2);
    gain.gain.value=cue.loop?0:cue.gain*clamp(settings.gain??1);
    if(pan)pan.pan.value=Math.max(-1,Math.min(1,Number(settings.pan)||0));
    source.connect(gain);if(pan){gain.connect(pan);pan.connect(master);}else gain.connect(master);
    const voice={source,gain,pan,cue};voices.add(voice);
    source.onended=()=>{
      if(!voices.delete(voice))return;
      source.disconnect();gain.disconnect();pan?.disconnect();
    };
    if(cue.loop){
      source.loop=true;source.loopStart=cue.offset;source.loopEnd=cue.offset+cue.duration;
      source.start(context.currentTime,cue.offset);
    }else source.start(context.currentTime,cue.offset??0,cue.duration??buffer.duration);
    return voice;
  };
  const refreshLoops=()=>{
    if(!audible()){stop();return;}
    const wanted=new Map((desired.loops||[]).filter(loop=>sampleCues[loop.name]?.[0]?.loop&&clamp(loop.gain??1)>0).map(loop=>[loop.name,loop]));
    for(const [name,voice] of loops)if(!wanted.has(name)){stopVoice(voice);loops.delete(name);}
    for(const [name,settings] of wanted){
      let voice=loops.get(name);
      if(!voice){
        voice=start(sampleCues[name][0],settings);
        if(!voice)continue;
        loops.set(name,voice);
      }
      voice.gain.gain.setTargetAtTime(voice.cue.gain*clamp(settings.gain??1),context.currentTime,.12);
      voice.source.playbackRate.setTargetAtTime(Math.max(.5,clamp(settings.rate||1,2)),context.currentTime,.12);
      voice.pan?.pan.setTargetAtTime(Math.max(-1,Math.min(1,Number(settings.pan)||0)),context.currentTime,.12);
    }
  };
  const load=async url=>{
    if(loading.has(url)||buffers.has(url)||!context)return;
    loading.add(url);
    try{
      for(let attempt=0;attempt<3;attempt++){
        try{
          const response=await fetch(url,{signal:AbortSignal.timeout(12000)});
          if(!response.ok)throw new Error('Audio unavailable');
          const bytes=await response.arrayBuffer();
          if(bytes.byteLength>4_000_000)throw new Error('Audio bank exceeds its budget');
          buffers.set(url,await context.decodeAudioData(bytes));
          refreshLoops();return;
        }catch{
          if(attempt<2)await new Promise(resolve=>setTimeout(resolve,[800,2400][attempt]));
        }
      }
    }finally{loading.delete(url);}
  };
  const unlock=()=>{
    try{
      if(!context){
        const Audio=window.AudioContext||window.webkitAudioContext;if(!Audio)return;
        context=new Audio();master=context.createGain();master.gain.value=desired.active&&!document.hidden?volume/100:0;master.connect(context.destination);
      }
      if(context.state==='suspended'||context.state==='interrupted')void context.resume().then(refreshLoops).catch(()=>{});
      void load(bankUrl);
      for(const url of recordingUrls)void load(url);
    }catch{ /* Missing audio never blocks gameplay. */ }
  };
  target.addEventListener('pointerdown',unlock,{capture:true});
  target.addEventListener('touchend',unlock,{capture:true,passive:true});
  target.addEventListener('keydown',unlock,{capture:true});
  if(typeof document.addEventListener==='function')document.addEventListener('visibilitychange',()=>{
    if(document.hidden){stop();if(master)master.gain.value=0;}
    // The next committed state decides whether the resumed scene is audible.
  });
  return {
    unlock,
    get volume(){return volume;},
    sync(state){
      const next=clamp(state.volume??volume,100);
      if(next!==volume&&storageKey)try{localStorage.setItem(storageKey,JSON.stringify({effects:next}));}catch{}
      volume=next;desired={active:Boolean(state.active),loops:state.loops||[]};
      if(master)master.gain.setTargetAtTime(desired.active&&!document.hidden?volume/100:0,context.currentTime,.01);
      refreshLoops();
      if(!audible())return;
      for(const effect of state.effects||[]){
        const settings=typeof effect==='string'?{name:effect}:effect;
        const name=settings.name,cue=choose(name,settings.variant);if(!cue||cue.loop)continue;
        // Keep bursts bounded without delaying an effect until it loses meaning.
        if(voices.size>=16)continue;
        start(cue,settings);
      }
    }
  };
}
