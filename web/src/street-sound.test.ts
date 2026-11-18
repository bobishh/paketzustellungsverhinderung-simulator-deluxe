import {describe,it,expect} from 'vitest';
import {engineLayers,StepDistance} from './street-sound';

describe('recorded street sound',()=>{
 it('changes engine load at the same road speed, then approaches idle when gas is released',()=>{
  const loaded=engineLayers(0,1),idle=engineLayers(0,0);
  expect(loaded.find(layer=>layer.name==='engine-low')!.gain).toBeGreaterThan(idle.find(layer=>layer.name==='engine-low')!.gain);
  expect(idle.find(layer=>layer.name==='engine-idle')!.gain).toBeGreaterThan(loaded.find(layer=>layer.name==='engine-idle')!.gain);
  for(const speed of [0,5,16,18,35,42,65,70,110,140])for(const throttle of [-1,0,1])
   for(const layer of engineLayers(speed,throttle)){
    expect(layer.gain).toBeGreaterThanOrEqual(0);expect(layer.gain).toBeLessThanOrEqual(1);
    expect(layer.rate).toBeGreaterThan(.5);expect(layer.rate).toBeLessThanOrEqual(2);
   }
 });
 it('emits footsteps from travelled ground distance, with no sound for idle, jumps or teleportation',()=>{
  const steps=new StepDistance();
  expect(steps.advance([0,0,0],false)).toBe(false);
  expect(steps.advance([.5,0,0],false)).toBe(false);
  expect(steps.advance([1,0,0],false)).toBe(true);
  expect(steps.advance([1,0,0],false)).toBe(false);
  expect(steps.advance([1,2,0],true)).toBe(false);
  expect(steps.advance([500,0,0],false)).toBe(false);
 });
});
