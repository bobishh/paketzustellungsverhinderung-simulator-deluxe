export type Point = [number,number];
export type Member = {id:string;name:string;slot:number};
export type Peer = {id:string;position:Point;form:'courier'|'dog'|'van';age:number};
export const PEER_TIMEOUT_MS = 10_000;
const hues=[195,305,145,265,55,220,335,175,285,105,240,315,75,205,155,255,355];
export function peerColor(slot:number) { return `hsl(${hues[slot%hues.length]}, 78%, 70%)`; }
export function readMembers(value:unknown):Member[] {
  if(!Array.isArray(value)||value.length>17)return [];
  const ids=new Set(),slots=new Set();
  for(const m of value){
    if(!m||typeof m.id!=='string'||!/^[a-f0-9]{64}$/.test(m.id)||typeof m.name!=='string'||!m.name.trim()||m.name.length>48||!Number.isInteger(m.slot)||m.slot<0||m.slot>16||ids.has(m.id)||slots.has(m.slot))return [];
    ids.add(m.id);slots.add(m.slot);
  }
  return value;
}
export function ringMarker(origin:Point,point:Point){
  const dx=point[0]-origin[0],dy=point[1]-origin[1],distance=Math.hypot(dx,dy),onRing=distance>=1;
  return {x:onRing?240+208*dx/distance:240,y:onRing?240+208*dy/distance:240,angle:Math.atan2(dx,-dy)*180/Math.PI,distance,onRing};
}
export type PoseReceipt={sequence:number;seen:number};
export function freshPose(poses:Map<string,PoseReceipt>,id:string,sequence:number,now:number):boolean {
  if(sequence<=(poses.get(id)?.sequence??-1))return false;
  poses.set(id,{sequence,seen:now});
  if(poses.size>256)poses.delete(poses.keys().next().value!);
  return true;
}
export const pollDelay=(started:number,finished:number)=>Math.max(0,100-(finished-started));
