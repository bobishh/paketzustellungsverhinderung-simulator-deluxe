import {describe,it,expect} from 'vitest';
import {deliveryGuidance,type GuidanceState,type DeliveryStop} from './guidance';
const stop:DeliveryStop={index:0,address:'Example 12',name:'Alex',distance:2,parcelDistance:null,rung:false,state:'in-van'};
const state:GuidanceState={selected:0,active:0,routes:[stop],ready:true,ended:false,form:'courier',photo:false,photoReady:false,bulk:null,vanDistance:3,stunSeconds:0};
describe('delivery guidance',()=>{
 it('starts another day in place instead of directing players to leave',()=>{
  const guidance=deliveryGuidance({...state,ended:true});
  expect(guidance.detail).toContain('vote for tomorrow');
  expect(guidance.action).toBe('Vote for the next day');
 });
 it('explains getting out and collecting the box after driving to a house',()=>{
  expect(deliveryGuidance({...state,form:'van'}).action).toContain('V · Leave van');
  expect(deliveryGuidance(state).action).toBe('E · Collect this parcel');
  expect(deliveryGuidance({...state,vanDistance:30}).action).toContain('Return to your van · 30 m');
 });
 it('does not suggest dropping before the actual bell is pressed',()=>{
  const carrying={...state,routes:[{...stop,state:'carrying' as const}]};
  expect(deliveryGuidance(carrying).detail).toContain('click the bell');
  expect(deliveryGuidance({...carrying,routes:[{...carrying.routes[0],rung:true}]}).action).toContain('T · Throw');
 });
 it('distinguishes airborne parcels from photographable boxes',()=>{
  expect(deliveryGuidance({...state,routes:[{...stop,state:'in-flight'}]}).action).toContain('Wait');
  expect(deliveryGuidance({...state,routes:[{...stop,state:'photo-due'}],photo:true,photoReady:true}).action).toBe('Space · Take delivery photo');
  expect(deliveryGuidance({...state,photo:true,routes:[{...stop,state:'carrying',rung:true}]}).action).toBe('F · Put camera away');
 });
 it('explains a different selected address without pretending the held box changed',()=>{
  expect(deliveryGuidance({...state,selected:1,routes:[{...stop,state:'carrying'}, {...stop,index:1}]}).detail).toContain('Example 12');
 });
 it('gives bite recovery precedence over parcel commands',()=>{
  expect(deliveryGuidance({...state,stunSeconds:1.2}).action).toBe('OUCH! · 1.2s');
 });
});
