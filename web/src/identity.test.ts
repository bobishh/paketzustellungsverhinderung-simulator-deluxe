import { beforeEach, expect, it, vi } from 'vitest';
import { BrowserIdentityStore, createDeviceAuthorizationApproval, toBase64Url } from '@meta-uber/mesh-identity';
function storage() { const data = new Map<string, string>(); return { getItem: (key: string) => data.get(key) ?? null, setItem: (key: string, value: string) => { data.set(key, value); }, removeItem: (key: string) => { data.delete(key); } }; }
let assigned: string;
const labels = new Map<string, { textContent: string }>();
beforeEach(() => {
  vi.resetModules(); assigned = ''; labels.clear();
  vi.stubGlobal('localStorage', storage()); vi.stubGlobal('sessionStorage', storage());
  vi.stubGlobal('location', { origin: 'https://game.example', search: '', hash: '#join=keep-invitation', assign: (url: string) => { assigned = url; } });
  vi.stubGlobal('document', { querySelector: (selector: string) => { if (!labels.has(selector)) labels.set(selector, { textContent: '' }); return labels.get(selector); } });
});
async function approvalFlow() {
  const client = await import('./identity');
  await client.restoreIdentity(null);
  const appDevice = client.profile.device.deviceId;
  await client.signIn();
  const pending = JSON.parse(sessionStorage.getItem('paket.identity.pending')!);
  const owner = await new BrowserIdentityStore({ storageKey: 'test-owner', storage: storage() }).bootstrap('Test owner');
  const approval = await createDeviceAuthorizationApproval(owner, pending.request, { applicationOrigin: location.origin,
    challenge: pending.request.payload.challenge, certificates: [owner.certificate] });
  const callback = (value = approval, state = pending.request.payload.challenge) => '#' + new URLSearchParams({ state, approval: toBase64Url(new TextEncoder().encode(JSON.stringify(value))) });
  return { client, owner, appDevice, approval, callback };
}
it('keeps the app key, restores the invitation and verifies identity again after reload', async () => {
  const f = await approvalFlow();
  expect(new URL(assigned).origin).toBe('https://identity.meta-uber-engineer.dev');
  expect(new URL(assigned).search).toBe('');
  expect(await f.client.restoreIdentity(f.callback())).toBe('/#join=keep-invitation');
  expect(f.client.profile.identity.personId).toBe(f.owner.identity.personId);
  expect(f.client.profile.device.deviceId).toBe(f.appDevice);
  expect(f.client.profile.privateKeys.identityPrivateKey).toBeUndefined();
  expect(sessionStorage.getItem('paket.identity.pending')).toBeNull();
  vi.resetModules(); const reloaded = await import('./identity');
  await reloaded.restoreIdentity(null);
  expect(reloaded.profile.identity.personId).toBe(f.owner.identity.personId);
  expect(reloaded.profile.device.deviceId).toBe(f.appDevice);
});
it('rejects mismatched state and cannot replay a consumed attempt', async () => {
  const f = await approvalFlow();
  await expect(f.client.restoreIdentity(f.callback(f.approval, 'wrong'))).rejects.toThrow('did not match');
  await expect(f.client.restoreIdentity(f.callback())).rejects.toThrow('expired');
  expect(localStorage.getItem('paket.identity.approval')).toBeNull();
});
it('rejects forged approval before adopting the identity', async () => {
  const f = await approvalFlow();
  const forged = structuredClone(f.approval); forged.proof.signature = 'a'.repeat(86);
  await expect(f.client.restoreIdentity(f.callback(forged))).rejects.toThrow();
  expect(f.client.profile.identity.personId).not.toBe(f.owner.identity.personId);
  expect(localStorage.getItem('paket.identity.approval')).toBeNull();
});
it('can still join a ride after cancelling sign-in', async () => {
  const f = await approvalFlow();
  const pending = JSON.parse(sessionStorage.getItem('paket.identity.pending')!);
  vi.resetModules();
  const returned = await import('./identity');
  expect(await returned.restoreIdentity('#' + new URLSearchParams({state: pending.request.payload.challenge, cancelled: '1'}))).toBe('/#join=keep-invitation');
  expect(returned.profile.device.deviceId).toBe(f.appDevice);
  expect(returned.certificates).toHaveLength(1);
});
