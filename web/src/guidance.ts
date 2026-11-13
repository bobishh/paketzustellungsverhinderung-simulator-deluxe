export type DeliveryStop = {
  index:number; address:string; name:string; distance:number; parcelDistance:number|null; rung:boolean; point?:[number,number];
  state:'in-van'|'carrying'|'in-flight'|'photo-due'|'filed'|'handed';
};
export type GuidanceState = {
  selected:number; active:number; routes:DeliveryStop[]; ready:boolean; ended:boolean;
  form:string; photo:boolean; photoReady:boolean; bulk:boolean|null; vanDistance:number|null; stunSeconds:number;
};
export function deliveryGuidance(s:GuidanceState) {
  const parcel=s.routes[s.active];
  const result=(step:string,detail:string,action:string)=>({step,detail,action});
  if(!s.ready)return result('Joining shift','Waiting for the shared clock.','Joining the shift…');
  if(s.ended)return result('Day complete','Compare scores and vote for tomorrow’s neighbourhood.','Vote for the next day');
  if(s.stunSeconds>0)return result('Bitten · recovering','A short shock. Your parcel and KPI are safe.',`OUCH! · ${s.stunSeconds.toFixed(1)}s`);
  if(s.form==='dog')return result('Cause trouble','Bite nearby couriers or scratch a closed door.','E · Bite / scratch     Space · Jump     G · Mark a van');
  if(s.form==='van')return result('1 / 4 · Get the parcel','Park at the orange delivery marker. Exit, then collect the selected parcel beside your van.','V · Leave van     then E · Collect parcel');
  if(typeof s.bulk==='boolean')return result('Delivery of the day',s.bulk?'Follow the marker to abandon the batch.':'Return to your van to load the whole batch.',s.bulk?'E · Abandon the batch':'E · Collect the whole batch');
  if(!parcel)return result('Finding addresses','The route fills as nearby streets load.','Loading your route…');
  if(s.active!==s.selected&&['carrying','in-flight','photo-due'].includes(parcel.state))return result('You are inspecting another stop',`Your current parcel is for ${parcel.address}. Select that stop to follow its marker, or return the held box to your van with Q.`,'Tab · Switch stop     Q near van · Return held box');
  if(s.photo&&parcel.state!=='photo-due'&&parcel.state!=='in-flight')return result('Put the camera away','Press F before collecting, placing or throwing a parcel.','F · Put camera away');
  if(parcel.state==='in-van')return result('1 / 4 · Get the parcel','Arriving at the house is not enough: the box is still in your van.',s.vanDistance!==null&&s.vanDistance<=3.7?'E · Collect this parcel':`Return to your van${s.vanDistance===null?'':` · ${Math.round(s.vanDistance)} m`} · then E`);
  if(parcel.state==='in-flight')return result('3 / 4 · Parcel airborne','Wait for it to stop. The marker follows your box.','Wait for the parcel to land');
  if(parcel.state==='carrying'&&!parcel.rung)return result('2 / 4 · Ring the recipient','Walk to the marked entrance. Press R, click the bell beside the recipient’s name, then Esc to close.','R · Open intercom     then click the recipient’s bell');
  if(parcel.state==='carrying')return result('3 / 4 · Leave it somewhere','E places it at a doorstep. T throws it ahead. If the recipient catches you, E hands it over for 0 KPI.','E · Place / hand over     T · Throw');
  if(parcel.state==='photo-due')return result('4 / 4 · Photograph the box','Face the box within 9 m with a clear view. Centre it in the camera; Space saves the proof and selects the next delivery.',s.photo?(s.photoReady?'Space · Take delivery photo':'Aim at the box · F to put camera away'):'F · Open camera     E nearby · Pick up');
  return result('Delivery complete','The next unfinished delivery is selected automatically.','Tab · Inspect another stop');
}
