import { IrohMeshNode, waitForRelayUrl } from '@meta-uber/mesh-transport';
import { profile, certificates } from './identity';
import { signEnvelope, verifyEnvelope, verifyDeviceCertificateChain } from '@meta-uber/mesh-identity';

type Origin = { lat: number; lon: number };
type Fact = { kind: string; origin: Origin; device?: string; sequence?: number; [key: string]: unknown };
type Invite = { protocol: 'paket-web/1'; origin: Origin; host: string; relay: string; room: string };
type State = { node: IrohMeshNode; invite: Invite; host: boolean; closed: boolean; inbox: Fact[]; latest: Map<string, Fact>; pending: Fact[]; timer?: ReturnType<typeof setTimeout> };
const sessions = new Map<string, State>();
export function decodeInvite(value: string): Invite {
  const encoded = value.includes('#join=') ? value.split('#join=')[1] : null;
  const invite = JSON.parse(encoded ? new TextDecoder().decode(Uint8Array.from(atob(encoded.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0))) : value);
  if (invite.protocol !== 'paket-web/1' || !/^[a-f0-9]{64}$/.test(invite.host) || !/^[a-f0-9]{64}$/.test(invite.room) ||
      !Number.isFinite(invite.origin?.lat) || Math.abs(invite.origin.lat) > 85 || !Number.isFinite(invite.origin?.lon) || Math.abs(invite.origin.lon) > 180 ||
      typeof invite.relay !== 'string' || new URL(invite.relay).protocol !== 'https:') throw new Error('Invalid browser ride invitation');
  return invite;
}
export function validFact(fact: Fact, origin: Origin, device?: string): boolean {
  if (!fact || !fact.origin || Math.abs(fact.origin.lat - origin.lat) > 1e-9 || Math.abs(fact.origin.lon - origin.lon) > 1e-9 ||
      !Number.isFinite(fact.origin.lat) || !Number.isFinite(fact.origin.lon) || JSON.stringify(fact).length > 4096) return false;
  if (!['actor', 'pose', 'delivery', 'streetAction'].includes(fact.kind)) return false;
  if (fact.kind !== 'delivery' && (typeof fact.device !== 'string' || (device !== undefined && fact.device !== device))) return false;
  const label = (value: unknown) => typeof value === 'string' && value.length > 0 && value.length <= 256;
  if (fact.kind === 'delivery') return label(fact.building_id);
  if (!Number.isSafeInteger(fact.sequence) || fact.sequence! < 0) return false;
  const finite = (value: any, fields: string[]) => value && fields.every(key => typeof value[key] === 'number' && Number.isFinite(value[key]) && Math.abs(value[key]) <= 1e7);
  if (fact.kind === 'pose') return !!finite(fact, ['east', 'north', 'heading']);
  if (fact.kind === 'actor') {
    const p = fact.pose as any;
    return !!finite(p, ['east', 'north', 'up', 'heading', 'velocity_east', 'velocity_north']) &&
      ['courier', 'dog', 'van'].includes(p.avatar) && (p.parked_van == null || !!finite(p.parked_van, ['east', 'north', 'heading']));
  }
  const e = fact.event as any;
  return e?.action === 'door' ? label(e.building_id) : e?.action === 'bite' ? label(e.target) :
    e?.action === 'markVan' && label(e.owner) && !!finite(e, ['east', 'north']);
}
function retain(s: State, fact: Fact) {
  const key = fact.kind === 'actor' || fact.kind === 'pose' ? `pose:${fact.device}` : `${fact.kind}:${fact.device ?? ''}:${fact.sequence ?? fact.building_id ?? ''}`;
  s.latest.set(key, fact);
  if (s.latest.size > 128) s.latest.delete(s.latest.keys().next().value!);
}
function incoming(s: State, fact: Fact) {
  retain(s, fact);
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
    const invite: Invite = bootstrap ?? { protocol: 'paket-web/1', origin, host: node.endpointId, relay,
      room: [...crypto.getRandomValues(new Uint8Array(32))].map(x => x.toString(16).padStart(2, '0')).join('') };
    const s: State = { node, invite, host, closed: false, inbox: [], latest: new Map(), pending: [] };
    const proof = await signEnvelope(profile.privateKeys.devicePrivateKey, { kind: 'paket-player', endpoint: node.endpointId,
      room: invite.room, expiresAt: Date.now() + 6 * 3600_000 }, profile.device.deviceId, 'PAKET/1');
    const auth = { identity: profile.identity, certificates, proof };
    const admitted = new Map<string, number>();
    if (host) {
      await node.listen(async (input: any) => {
        if (input?.protocol !== 'paket-web/1' || input.room !== invite.room || !Array.isArray(input.facts) || input.facts.length > 32) throw new Error('Invalid ride message');
        const endpoint = input.auth?.proof?.payload?.endpoint;
        if (typeof endpoint !== 'string' || !/^[a-f0-9]{64}$/.test(endpoint)) throw new Error('Invalid peer');
        if ((admitted.get(endpoint) ?? 0) < Date.now()) {
          if (admitted.size >= 16 && !admitted.has(endpoint)) throw new Error('Ride is full');
          const a = input.auth;
          const key = await verifyDeviceCertificateChain(a.identity, a.proof.signerKeyId, a.certificates);
          if (a.proof.payload.kind !== 'paket-player' || a.proof.payload.room !== invite.room || a.proof.payload.expiresAt <= Date.now() ||
              a.proof.payload.expiresAt > Date.now() + 6 * 3600_000 || !await verifyEnvelope(a.proof, key, 'PAKET/1')) throw new Error('Invalid rider identity');
          admitted.set(endpoint, Math.min(Date.now() + 60_000, a.proof.payload.expiresAt));
        }
        for (const fact of input.facts) { if (!validFact(fact, invite.origin, endpoint)) throw new Error('Invalid game fact'); incoming(s, fact); }
        return { protocol: 'paket-web/1', facts: [...s.latest.values()].filter(fact => fact.device !== endpoint).slice(-128) };
      });
    } else {
      const tick = async () => {
        if (s.closed) return;
        const batch = s.pending.splice(0, 32);
        try {
          const reply: any = await node.request(invite.host, { protocol: 'paket-web/1', room: invite.room, auth, facts: batch }, 4000);
          if (reply?.protocol !== 'paket-web/1' || !Array.isArray(reply.facts) || reply.facts.length > 128) throw new Error('Invalid host reply');
          for (const fact of reply.facts) if (validFact(fact, invite.origin)) incoming(s, fact);
          globalThis.dispatchEvent(new CustomEvent('paket-network', { detail: 'Connected' }));
        } catch {
          // Keep one-shot actions during a brief relay outage. Pose updates are replaceable.
          s.pending = [...batch.filter(f => f.kind !== 'actor' && f.kind !== 'pose'), ...s.pending].slice(-64);
          globalThis.dispatchEvent(new CustomEvent('paket-network', { detail: 'Reconnecting…' }));
        }
        if (!s.closed) s.timer = setTimeout(tick, 100);
      };
      s.timer = setTimeout(tick, 0);
    }
    sessions.set(node.endpointId, s);
    const json = JSON.stringify(invite);
    return JSON.stringify({ origin: invite.origin, device: node.endpointId, invitation: json, is_host: host });
  },
  publish(id: string, fact: Fact) {
    const s = sessions.get(id); if (!s || !validFact(fact, s.invite.origin, id)) return;
    retain(s, fact);
    if (!s.host) {
      if (fact.kind === 'actor' || fact.kind === 'pose') s.pending = s.pending.filter(f => f.kind !== 'actor' && f.kind !== 'pose');
      s.pending.push(fact); if (s.pending.length > 64) s.pending.shift();
    }
  },
  poll(id: string) { return sessions.get(id)?.inbox.splice(0) ?? []; },
  close(id: string) { const s = sessions.get(id); if (!s) return; s.closed = true; clearTimeout(s.timer); sessions.delete(id); void s.node.close(); },
};
