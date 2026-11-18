import {readFileSync} from 'node:fs';
import {afterEach,expect,it,vi} from 'vitest';

afterEach(()=>vi.useRealTimers());

it('retries the shipped audio bank after three failures when the player makes another gesture',async()=>{
 vi.useFakeTimers();
 const target=Object.assign(new EventTarget(),{hidden:false});
 const fetchBank=vi.fn()
  .mockRejectedValueOnce(new Error('offline'))
  .mockRejectedValueOnce(new Error('offline'))
  .mockRejectedValueOnce(new Error('offline'))
  .mockResolvedValue({ok:true,arrayBuffer:async()=>new ArrayBuffer(16)});
 const decode=vi.fn(async()=>({}));
 class Context {
  state='running';currentTime=0;destination={};
  decodeAudioData=decode;
  createGain(){return {gain:{value:0,setTargetAtTime(){}},connect(){}};}
 }
 // Evaluate the shipped module, replacing only its import/export wrappers.
 const source=readFileSync(new URL('../../assets/audio/job-sample-audio.js',import.meta.url),'utf8')
  .replace(/^import \{sampleCues\} from .*;$/m,'')
  .replace('export function createSampleAudio','return function createSampleAudio');
 const createPlayer=new Function('sampleCues','document','window','fetch','AbortSignal','setTimeout',source)(
  {},target,{AudioContext:Context},fetchBank,{timeout:()=>new AbortController().signal},setTimeout,
 );
 createPlayer({target});
 target.dispatchEvent(new Event('pointerdown'));
 target.dispatchEvent(new Event('pointerdown'));
 expect(fetchBank).toHaveBeenCalledTimes(1);
 await vi.runAllTimersAsync();
 expect(fetchBank).toHaveBeenCalledTimes(3);
 expect(decode).not.toHaveBeenCalled();
 target.dispatchEvent(new Event('pointerdown'));
 await vi.runAllTimersAsync();
 expect(fetchBank).toHaveBeenCalledTimes(4);
 expect(decode).toHaveBeenCalledTimes(1);
 target.dispatchEvent(new Event('pointerdown'));
 expect(fetchBank).toHaveBeenCalledTimes(4);
});
