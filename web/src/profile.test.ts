import { expect, it, vi } from 'vitest';
import { BrowserIdentityStore, signEnvelope } from '@meta-uber/mesh-identity';
import { acceptProfile, profileBatch } from './profile';
it('replicates signed game careers and rejects forgery, rollback and foreign scope', async () => {
  vi.stubGlobal('location',{origin:'https://game.example'});
  const data=new Map<string,string>();
  const storage={getItem:(k:string)=>data.get(k)??null,setItem:(k:string,v:string)=>{data.set(k,v);},removeItem:(k:string)=>{data.delete(k);}};
  const owner=await new BrowserIdentityStore({storageKey:'career-test',storage}).bootstrap('Player');
  const payload={kind:'paket-career' as const,application:location.origin,personId:owner.identity.personId,deviceId:owner.device.deviceId,version:1,joinedAt:Date.now(),face:123,faceUpdated:Date.now(),stats:{workMs:1000,shifts:1,delivered:1,photos:1,score:100,rings:1,markedVans:0,special:0}};
  const sign=(value:typeof payload)=>signEnvelope(owner.privateKeys.devicePrivateKey,value,owner.device.deviceId,'PAKET-PROFILE/1');
  const record={identity:owner.identity,certificates:[owner.certificate],signed:await sign(payload)};
  await acceptProfile(record,false);await acceptProfile(record,false);expect(profileBatch()).toHaveLength(1);
  const forged=structuredClone(record);forged.signed.payload.version=2;forged.signed.payload.stats.score=99999;
  await expect(acceptProfile(forged,false)).rejects.toThrow('signature');
  await expect(acceptProfile({...record,signed:await sign({...payload,version:2,stats:{...payload.stats,score:0}})},false)).rejects.toThrow('backwards');
  await expect(acceptProfile({...record,signed:await sign({...payload,application:'https://other.example',version:2})},false)).rejects.toThrow('Invalid career');
  expect(profileBatch()[0].signed.payload.stats.score).toBe(100);
});
