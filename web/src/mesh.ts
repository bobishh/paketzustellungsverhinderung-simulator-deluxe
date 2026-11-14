import {freshPose, pollDelay, readMembers, PEER_TIMEOUT_MS, type Member, type PoseReceipt} from './peers';
import { publishMesh } from './mesh-directory';
import { IrohMeshNode, waitForRelayUrl } from '@meta-uber/mesh-transport';
import { profile, certificates } from './identity';
import { profileBatch, acceptProfile } from './profile';
import { signEnvelope, verifyEnvelope, verifyDeviceCertificateChain } from '@meta-uber/mesh-identity';

type Origin = { lat: number; lon: number };
type Fact = { kind: string; origin: Origin; device?: string; sequence?: number; [key: string]: unknown };
type Invite = { protocol: 'paket-web/2'; origin: Origin; host: string; relay: string; room: string };
type Presence = Member & {seen:number;until:number};
type State = { poses:Map<string,PoseReceipt>; members:Map<string,Presence>; memberVersion?:string; node: IrohMeshNode; invite: Invite; host: boolean; closed: boolean; inbox: Fact[]; latest: Map<string, Fact>; pending: Fact[]; unlist?: () => void; timer?: ReturnType<typeof setTimeout> };
const sessions = new Map<string, State>();
export function decodeInvite(value: string): Invite {
  const encoded = value.includes('#join=') ? value.split('#join=')[1] : null;
  const invite = JSON.parse(encoded ? new TextDecoder().decode(Uint8Array.from(atob(encoded.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0))) : value);
  if(invite.protocol==='paket-web/1')throw new Error('This invitation uses an older game version. Reload the host and create a new mesh.');
  if (invite.protocol !== 'paket-web/2' || !/^[a-f0-9]{64}$/.test(invite.host) || !/^[a-f0-9]{64}$/.test(invite.room) ||
      !Number.isFinite(invite.origin?.lat) || Math.abs(invite.origin.lat) > 85 || !Number.isFinite(invite.origin?.lon) || Math.abs(invite.origin.lon) > 180 ||
      typeof invite.relay !== 'string' || new URL(invite.relay).protocol !== 'https:') throw new Error('Invalid browser ride invitation');
  return invite;
}
export function inviteLink(invite: Invite, base: string): string {
  const canonical = JSON.stringify({protocol:invite.protocol,origin:{lat:invite.origin.lat,lon:invite.origin.lon},host:invite.host,relay:invite.relay,room:invite.room});
  const encoded = btoa(String.fromCharCode(...new TextEncoder().encode(canonical))).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');
  return `${base}/#join=${encoded}`;
}
export function validFact(fact: Fact, origin: Origin, device?: string): boolean {
  if (!fact || !fact.origin || Math.abs(fact.origin.lat - origin.lat) > 1e-9 || Math.abs(fact.origin.lon - origin.lon) > 1e-9 ||
      !Number.isFinite(fact.origin.lat) || !Number.isFinite(fact.origin.lon) || JSON.stringify(fact).length > (fact.kind==='route'?16384:4096)) return false;
  if (!['actor', 'pose', 'delivery', 'delivered', 'route', 'streetAction', 'shift', 'ballot'].includes(fact.kind)) return false;
  if (fact.kind !== 'delivery' && (typeof fact.device !== 'string' || (device !== undefined && fact.device !== device))) return false;
  const day = fact.day === undefined ? 1 : Number(fact.day);
  if (!Number.isSafeInteger(day) || day < 1 || day > 36_500) return false;
  const point=(o:any)=>o&&typeof o.lat==='number'&&Number.isFinite(o.lat)&&Math.abs(o.lat)<=85&&typeof o.lon==='number'&&Number.isFinite(o.lon)&&Math.abs(o.lon)<=180;
  if (fact.kind === 'shift') return Number.isSafeInteger(fact.elapsed_ms) && Number(fact.elapsed_ms) >= 0 && Number(fact.elapsed_ms) <= 2_880_000 && [0,1,2].includes(fact.difficulty as number)
    && (fact.world==null||point(fact.world)) && (fact.choices===undefined||(Array.isArray(fact.choices)&&[0,3].includes(fact.choices.length)&&fact.choices.every(point)))
    && (fact.remaining_ms===undefined||(Number.isSafeInteger(fact.remaining_ms)&&Number(fact.remaining_ms)>=0&&Number(fact.remaining_ms)<=20000));
  if (fact.kind === 'streetAction' && fact.day !== undefined && (!Number.isSafeInteger(fact.day) || Number(fact.day) < 1 || Number(fact.day) > 36_501)) return false;
  const label = (value: unknown) => typeof value === 'string' && value.length > 0 && value.length <= 256;
  if (fact.kind === 'delivery') return label(fact.building_id);
  const receipt=(r:any)=>r&&typeof r.device==='string'&&/^[a-f0-9]{64}$/.test(r.device)&&typeof r.photo==='boolean'&&[0,25,100].includes(r.score)&&(r.photo||r.score===0);
  if(fact.kind==='delivered')return label(fact.building_id)&&receipt(fact);
  if(fact.kind==='route'){
    if(!Number.isSafeInteger(fact.revision)||Number(fact.revision)<0||!Array.isArray(fact.stops)||fact.stops.length>12)return false;
    const ids=new Set();
    return fact.stops.every((s:any)=>{if(!s||!label(s.building_id)||ids.has(s.building_id)||!label(s.label)||!Array.isArray(s.position)||s.position.length!==3||!s.position.every((n:unknown)=>typeof n==='number'&&Number.isFinite(n)&&Math.abs(n)<=1e7)||(s.completion!==null&&!receipt(s.completion)))return false;ids.add(s.building_id);return true;});
  }
  if (!Number.isSafeInteger(fact.sequence) || fact.sequence! < 0) return false;
  if (fact.kind==='ballot')return typeof fact.courier==='boolean'&&typeof fact.finished==='boolean'&&Number.isSafeInteger(fact.score)&&Math.abs(Number(fact.score))<=1_000_000&&Number.isSafeInteger(fact.delivered)&&Number(fact.delivered)>=0&&Number(fact.delivered)<=100&&(fact.choice==null||[0,1,2].includes(fact.choice as number));
  const finite = (value: any, fields: string[]) => value && fields.every(key => typeof value[key] === 'number' && Number.isFinite(value[key]) && Math.abs(value[key]) <= 1e7);
  if (fact.kind === 'pose') return !!finite(fact, ['east', 'north', 'heading']);
  if (fact.kind === 'actor') {
    const p = fact.pose as any;
    return !!finite(p, ['east', 'north', 'up', 'heading', 'velocity_east', 'velocity_north']) &&
      ['courier', 'dog', 'van'].includes(p.avatar) && (p.parked_van == null || (!!finite(p.parked_van, ['east', 'north', 'heading']) && (p.parked_van.up === undefined || !!finite(p.parked_van, ['up']))));
  }
  const e = fact.event as any;
  return e?.action === 'door' ? label(e.building_id) : e?.action === 'bite' ? label(e.target) :
    e?.action === 'markVan' && label(e.owner) && !!finite(e, ['east', 'north']);
}
function roster(s:State):Member[] { return [...s.members.values()].map(({id,name,slot})=>({id,name,slot})).sort((a,b)=>a.slot-b.slot); }
function emitMembers(s:State) {
  const members=roster(s),version=JSON.stringify(members);
  if(s.memberVersion===version)return;
  s.memberVersion=version;globalThis.dispatchEvent(new CustomEvent('paket-members',{detail:members}));
}
function acceptMembers(s:State,value:unknown) {
  if(value===undefined)return;
  const members=readMembers(value);
  if(!members.length)return;
  s.members=new Map(members.map(m=>[m.id,{...m,seen:Date.now(),until:0}]));
}
function prune(s:State) {
  const now=Date.now();
  for(const [id,receipt] of s.poses)if(now-receipt.seen>PEER_TIMEOUT_MS)s.latest.delete(`pose:${id}`);
  for(const [id,m] of s.members)if(id!==s.node.endpointId&&now-m.seen>PEER_TIMEOUT_MS)s.members.delete(id);
  emitMembers(s);
}
function retain(s: State, fact: Fact) {
  if((fact.kind==='actor'||fact.kind==='pose')&&!freshPose(s.poses,fact.device!,fact.sequence!,Date.now()))return false;
  const key = fact.kind === 'route' ? 'route' : fact.kind === 'delivered'?`delivered:${fact.day}:${fact.device}:${fact.building_id}`:fact.kind === 'shift' ? 'shift' : fact.kind==='ballot'?`ballot:${fact.device}`:fact.kind === 'actor' || fact.kind === 'pose' ? `pose:${fact.device}` : `${fact.kind}:${fact.device ?? ''}:${fact.sequence ?? fact.building_id ?? ''}`;
  s.latest.set(key, fact);
  if(fact.kind==='route')for(const [key,stored] of s.latest)if(stored.kind==='delivered'&&Number(stored.day)<Number(fact.day))s.latest.delete(key);
  if (s.latest.size > 128) {
    const discard=[...s.latest.keys()].find(k=>k!=='route'&&k!=='shift'&&!k.startsWith('pose:')&&!k.startsWith('ballot:'));
    if(discard)s.latest.delete(discard);
  }
  return true;
}
function incoming(s: State, fact: Fact) {
  if(!retain(s, fact))return;
  s.inbox.push(fact);
  if (s.inbox.length > 256) s.inbox.shift();
}
export const mesh = {
  async connect(origin: Origin, invitation?: string) {
    if (!profile) throw new Error('Identity is not ready');
    const bootstrap = invitation ? decodeInvite(invitation) : null;
    // Use the existing MetaMesh browser runtime; endpoint and private identity keys remain local.
    const moduleUrl = '/mesh/meta_mesh.js';
    const transport = await IrohMeshNode.loadModule(moduleUrl);
    const raw = await transport.BrowserNode.start(crypto.getRandomValues(new Uint8Array(32)));
    const routed = { endpointId: raw.endpointId, relayUrl: raw.relayUrl,
      waitForRelayUrl: () => raw.waitForRelayUrl(), accept: () => raw.accept(), close: (reason?: string) => raw.close(reason),
      dialRelay: (endpoint: string, relayUrl?: string) => raw.dialRelay(endpoint, relayUrl ?? (endpoint === bootstrap?.host ? bootstrap.relay : undefined)),
    };
    const node = new IrohMeshNode(routed, undefined, transport);
    let relay: string;
    try { relay = await waitForRelayUrl(raw); } catch (error) { await node.close(); throw error; }
    const host = !invitation;
    const ownerToken = [...crypto.getRandomValues(new Uint8Array(32))].map(x=>x.toString(16).padStart(2,'0')).join('');
    const room = [...new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(ownerToken)))].map(x=>x.toString(16).padStart(2,'0')).join('');
    const invite: Invite = bootstrap ?? { protocol: 'paket-web/2', origin, host: node.endpointId, relay,
      room };
    const s: State = { poses:new Map(),members:new Map(), node, invite, host, closed: false, inbox: [], latest: new Map(), pending: [] };
    const name=String(profile.identity.displayName||'Courier').trim().slice(0,48)||'Courier';
    if(host)s.members.set(node.endpointId,{id:node.endpointId,name,slot:0,seen:Date.now(),until:Infinity});
    const proof = await signEnvelope(profile.privateKeys.devicePrivateKey, { kind: 'paket-player', endpoint: node.endpointId, name,
      room: invite.room, expiresAt: Date.now() + 6 * 3600_000 }, profile.device.deviceId, 'PAKET/1');
    const auth = { identity: profile.identity, certificates, proof };
    let profileCursor = 0;
    if (host) {
      await node.listen(async (input: any) => {
        if (input?.protocol !== 'paket-web/2' || input.room !== invite.room || !Array.isArray(input.facts) || input.facts.length > 32) throw new Error('Invalid ride message');
        const endpoint = input.auth?.proof?.payload?.endpoint;
        if (typeof endpoint !== 'string' || !/^[a-f0-9]{64}$/.test(endpoint)) throw new Error('Invalid peer');
        prune(s);
        const member=s.members.get(endpoint);
        if ((member?.until ?? 0) < Date.now()) {
          if (s.members.size >= 17 && !member) throw new Error('Ride is full');
          const a = input.auth;
          const key = await verifyDeviceCertificateChain(a.identity, a.proof.signerKeyId, a.certificates);
          if (a.proof.payload.kind !== 'paket-player' || a.proof.payload.room !== invite.room || a.proof.payload.expiresAt <= Date.now() ||
              a.proof.payload.expiresAt > Date.now() + 6 * 3600_000 || !await verifyEnvelope(a.proof, key, 'PAKET/1')) throw new Error('Invalid rider identity');
          const used=new Set([...s.members.values()].map(m=>m.slot));
          const slot=member?.slot??Array.from({length:16},(_,i)=>i+1).find(i=>!used.has(i))!;
          const name=String(a.proof.payload.name||a.identity.displayName||'Courier').trim().slice(0,48)||'Courier';
          s.members.set(endpoint,{id:endpoint,name,slot,seen:Date.now(),until:Math.min(Date.now()+60_000,a.proof.payload.expiresAt)});
        }
        s.members.get(endpoint)!.seen=Date.now();
        emitMembers(s);
        for (const fact of input.facts) { if ((fact.kind === 'shift'||fact.kind==='route') || !validFact(fact, invite.origin, endpoint)) throw new Error('Invalid game fact'); incoming(s, fact); }
        if (Array.isArray(input.profiles) && input.profiles.length <= 2) for (const record of input.profiles) { try { await acceptProfile(record); } catch { /* Reject unsigned or foreign application data. */ } }
        return { members:roster(s), profiles: profileBatch(profileCursor++), protocol: 'paket-web/2', facts: [...s.latest.values()].filter(fact => fact.device !== endpoint).slice(-128) };
      });
    } else {
      // Do not enter a shift until the host has accepted the invitation.
      try {
        const hello: any = await node.request(invite.host, {protocol:'paket-web/2',room:invite.room,auth,facts:[],profiles:[]}, 8000);
        if (hello?.protocol !== 'paket-web/2') throw new Error('Invalid host reply');
        acceptMembers(s,hello.members);
      } catch (error) { await node.close(); throw error; }
      const tick = async () => {
        if (s.closed) return;
        const started=performance.now();
        const batch = s.pending.splice(0, 32);
        try {
          const reply: any = await node.request(invite.host, { protocol: 'paket-web/2', room: invite.room, auth, facts: batch, profiles: profileBatch(profileCursor++) }, 4000);
          if (reply?.protocol !== 'paket-web/2' || !Array.isArray(reply.facts) || reply.facts.length > 128) throw new Error('Invalid host reply');
          acceptMembers(s,reply.members);emitMembers(s);
          if (Array.isArray(reply.profiles) && reply.profiles.length <= 2) for (const record of reply.profiles) { try { await acceptProfile(record); } catch { /* Verify each profile independently of the host. */ } }
          for (const fact of reply.facts) if (validFact(fact, invite.origin)) incoming(s, fact);
          globalThis.dispatchEvent(new CustomEvent('paket-network', { detail: 'Connected' }));
        } catch {
          // Keep one-shot actions during a brief relay outage. Pose updates are replaceable.
          s.pending = [...batch.filter(f => f.kind !== 'actor' && f.kind !== 'pose'), ...s.pending].slice(-64);
          globalThis.dispatchEvent(new CustomEvent('paket-network', { detail: 'Reconnecting…' }));
        }
        if (!s.closed) s.timer = setTimeout(tick, pollDelay(started,performance.now()));
      };
      s.timer = setTimeout(tick, 0);
    }
    if (host && globalThis.paketPublicMesh) s.unlist = publishMesh(invite, ownerToken);
    sessions.set(node.endpointId, s);
    const json = JSON.stringify(invite);
    const link = inviteLink(invite, location.origin);
    globalThis.dispatchEvent(new CustomEvent('paket-session', {detail: {link, host,device:node.endpointId}}));
    emitMembers(s);
    return JSON.stringify({ origin: invite.origin, device: node.endpointId, invitation: json, is_host: host });
  },
  publish(id: string, fact: Fact) {
    const s = sessions.get(id); if (!s || !validFact(fact, s.invite.origin, id)) return;
    prune(s);
    if(!retain(s, fact))return;
    if (!s.host) {
      if (fact.kind === 'actor' || fact.kind === 'pose') s.pending = s.pending.filter(f => f.kind !== 'actor' && f.kind !== 'pose');
      s.pending.push(fact); if (s.pending.length > 64) s.pending.shift();
    }
  },
  poll(id: string) { return sessions.get(id)?.inbox.splice(0) ?? []; },
  close(id: string) { const s = sessions.get(id); if (!s) return; s.closed = true; s.unlist?.(); clearTimeout(s.timer); sessions.delete(id); if (!sessions.size) globalThis.dispatchEvent(new CustomEvent('paket-session', {detail: null})); void s.node.close(); },
};
