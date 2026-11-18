// Shared street mix. Feed the Poor carries the same implementation.
export type Loop={name:string;gain:number;pan?:number;rate?:number};
export type Effect=string|{name:string;gain?:number;pan?:number;variant?:number};
export type SamplePlayer={unlock():void;sync(state:{active:boolean;volume:number;effects?:Effect[];loops?:Loop[]}):void};
export type StreetState={active:boolean;position:[number,number,number];speed:number;verticalSpeed:number;form:string;throttle:number;traffic:{distance:number;pan:number;speed:number}|null};
const clamp=(value:number)=>Math.max(0,Math.min(1,Number.isFinite(value)?value:0));

export function engineLayers(speed:number,throttle:number):Loop[]{
 const roadSpeed=Math.abs(Number.isFinite(speed)?speed:0),load=clamp(Math.abs(throttle));
 const gears=[0,18,40,70,110,160];
 const nextGear=gears.findIndex(limit=>limit>roadSpeed);
 const gear=nextGear<0?gears.length-1:Math.max(1,nextGear);
 const fraction=clamp((roadSpeed-gears[gear-1])/(gears[gear]-gears[gear-1]));
 const rpm=clamp(roadSpeed<2?load*.38:.22+fraction*.65+load*.08);
 const idle=1-clamp(rpm/.27),blend=clamp((rpm-.25)/.65),power=.4+load*.6;
 return [
  {name:'engine-idle',gain:.6*Math.sqrt(idle),rate:.9+rpm*.3},
  {name:'engine-low',gain:.8*(1-idle)*Math.sqrt(1-blend)*power,rate:.78+rpm*.55},
  {name:'engine-high',gain:.8*(1-idle)*Math.sqrt(blend)*power,rate:.74+rpm*.5},
 ];
}

export class StepDistance {
 private previous?:[number,number,number];private distance=0;
 reset(){this.previous=undefined;this.distance=0;}
 advance(position:[number,number,number],airborne:boolean,dog=false){
  const previous=this.previous;this.previous=position;
  if(!previous||airborne){this.distance=0;return false;}
  const distance=Math.hypot(position[0]-previous[0],position[2]-previous[2]);
  if(!Number.isFinite(distance)||distance>3){this.distance=0;return false;}
  this.distance+=distance;
  const stride=dog?.48:.8;
  if(this.distance<stride)return false;
  this.distance%=stride;return true;
 }
}

export function streetLoops(state:StreetState):Loop[]{
 const loops:Loop[]=[{name:'street',gain:.55}];
 if(state.form==='van')loops.push(...engineLayers(state.speed,state.throttle));
 const car=state.traffic;
 if(car&&car.distance<42)loops.push({name:'engine',gain:.45*Math.pow(1-car.distance/42,2),pan:car.pan,rate:.85+Math.min(1,car.speed/45)*.35});
 return loops;
}
