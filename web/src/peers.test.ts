import {expect,it} from 'vitest';
import {ringMarker, peerColor, readMembers, freshPose, pollDelay} from './peers';
it('puts near and distant players on the same circle with the correct bearing',()=>{
  expect(ringMarker([0,0],[100,0])).toMatchObject({x:448,y:240,distance:100,onRing:true});
  expect(ringMarker([0,0],[0,-5])).toMatchObject({x:240,y:32,distance:5});
  const diagonal=ringMarker([10,10],[1010,1010]);
  expect(Math.hypot(diagonal.x-240,diagonal.y-240)).toBeCloseTo(208);
  expect(ringMarker([0,0],[0,0]).onRing).toBe(false);
});
it('gives every admitted player a distinct stable color',()=>{
  const colors=Array.from({length:17},(_,i)=>peerColor(i));
  expect(new Set(colors).size).toBe(17);
  expect(peerColor(4)).toBe(colors[4]);
});
it('rejects malformed rosters and duplicate color slots',()=>{
  const a={id:'a'.repeat(64),name:'Alice',slot:0},b={id:'b'.repeat(64),name:'Bob',slot:1};
  expect(readMembers([a,b])).toEqual([a,b]);
  expect(readMembers([a,{...b,slot:0}])).toEqual([]);
  expect(readMembers([{...a,name:'x'.repeat(1000)}])).toEqual([]);
});
it('does not refresh presence or replay movement for duplicate and out-of-order poses',()=>{
  const poses=new Map();
  expect(freshPose(poses,'alice',10,100)).toBe(true);
  expect(freshPose(poses,'alice',10,200)).toBe(false);
  expect(freshPose(poses,'alice',9,300)).toBe(false);
  expect(poses.get('alice')).toEqual({sequence:10,seen:100});
  expect(freshPose(poses,'alice',11,400)).toBe(true);
});
it('includes request time in the polling interval',()=>{
  expect(pollDelay(1000,1040)).toBe(60);
  expect(pollDelay(1000,1250)).toBe(0);
});
