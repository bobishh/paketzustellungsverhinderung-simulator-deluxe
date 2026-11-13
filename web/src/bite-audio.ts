// A brief, procedural cartoon yelp. No microphone, downloads or speech service.
let context:AudioContext|undefined;
const toggle=document.querySelector<HTMLInputElement>('#game-sound')!;
try{toggle.checked=localStorage.getItem('paket.sound')!=='off';}catch{}
toggle.addEventListener('change',()=>{try{localStorage.setItem('paket.sound',toggle.checked?'on':'off');}catch{}});
function unlock(){
  if(!toggle.checked)return;
  try{context??=new AudioContext();if(context.state==='suspended')void context.resume().catch(()=>undefined);}catch{}
}
document.addEventListener('pointerdown',unlock);
document.addEventListener('keydown',unlock);
window.addEventListener('paket-bite',()=>{
  if(!toggle.checked||document.hidden||!context||context.state!=='running')return;
  const t=context.currentTime, voice=context.createOscillator(), throat=context.createBiquadFilter(), gain=context.createGain();
  voice.type='sawtooth';voice.frequency.setValueAtTime(190,t);voice.frequency.exponentialRampToValueAtTime(470,t+.07);voice.frequency.exponentialRampToValueAtTime(260,t+.23);voice.frequency.exponentialRampToValueAtTime(130,t+.48);
  throat.type='bandpass';throat.frequency.setValueAtTime(1050,t);throat.frequency.linearRampToValueAtTime(650,t+.5);throat.Q.value=1.5;
  gain.gain.setValueAtTime(0,t);gain.gain.linearRampToValueAtTime(.24,t+.025);gain.gain.setValueAtTime(.2,t+.18);gain.gain.exponentialRampToValueAtTime(.001,t+.52);
  voice.connect(throat).connect(gain).connect(context.destination);voice.start(t);voice.stop(t+.54);
  voice.onended=()=>{voice.disconnect();throat.disconnect();gain.disconnect();};
});
export {};
