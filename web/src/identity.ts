import {
  BrowserIdentityStore, createDeviceAuthorizationRequest, encodeDeviceAuthorizationRequest,
  parseDeviceAuthorizationApproval, verifyRetainedDeviceAuthorizationApproval,
  identityProfileValue, type DeviceAuthorizationRequest, type DeviceAuthorizationApproval, type LocalProfile,
} from '@meta-uber/mesh-identity';

const store = new BrowserIdentityStore({ storageKey: 'paket.identity', requirePersistence: true });
const PENDING = 'paket.identity.pending';
const EVIDENCE = 'paket.identity.approval';
type Pending = { request: DeviceAuthorizationRequest; returnPath: string };
export let profile: LocalProfile;
export let certificates: DeviceAuthorizationApproval['certificates'] = [];

export async function restoreIdentity(callbackHash: string | null): Promise<string | null> {
  profile = await store.bootstrap('Courier');
  if (callbackHash !== null) {
    const raw = sessionStorage.getItem(PENDING);
    sessionStorage.removeItem(PENDING);
    if (!raw) throw new Error('This sign-in has expired. Start Sign in again.');
    const pending: Pending = JSON.parse(raw);
    const result = new URLSearchParams(callbackHash.replace(/^#/, ''));
    if (pending.request.payload.applicationOrigin !== location.origin || result.get('state') !== pending.request.payload.challenge) {
      throw new Error('Sign-in did not match this browser.');
    }
    if (result.get('cancelled') === '1') {
      await restoreIdentity(null);
      return pending.returnPath;
    }
    const approval = await parseDeviceAuthorizationApproval(result.get('approval') ?? '', pending.request, { expectedExistingProfile: profile });
    if (approval.certificate.payload.deviceId !== profile.device.deviceId) throw new Error('Wrong application device.');
    // Verify before adoption; persist evidence before making the signed-in UI visible.
    const previous = localStorage.getItem(EVIDENCE);
    localStorage.setItem(EVIDENCE, JSON.stringify({ request: pending.request, approval }));
    try { profile = await store.adopt(approval.identity, approval.certificate); }
    catch (error) {
      if (previous === null) localStorage.removeItem(EVIDENCE); else localStorage.setItem(EVIDENCE, previous);
      throw error;
    }
    certificates = approval.certificates;
    renderIdentity(approval);
    return pending.returnPath;
  }
  const raw = localStorage.getItem(EVIDENCE);
  if (raw) {
    const saved = JSON.parse(raw);
    if (saved.request.payload.applicationOrigin !== location.origin) throw new Error('Saved identity belongs to another site.');
    const approval = await verifyRetainedDeviceAuthorizationApproval(saved.approval, saved.request);
    if (approval.identity.personId !== profile.identity.personId || approval.certificate.payload.deviceId !== profile.device.deviceId) {
      throw new Error('Saved identity does not match this device.');
    }
    certificates = approval.certificates;
    renderIdentity(approval);
  } else {
    certificates = [profile.certificate];
  }
  return null;
}
function renderIdentity(approval: DeviceAuthorizationApproval) {
  const signedName = approval.profileState && identityProfileValue(approval.profileState, 'name').values[0];
  document.querySelector('#identity-name')!.textContent = typeof signedName === 'string' ? signedName : approval.identity.displayName;
  document.querySelector('#signin')!.textContent = 'Switch Identity';
}
export async function signIn() {
  const current = await store.bootstrap('Courier');
  const challenge = [...crypto.getRandomValues(new Uint8Array(32))].map(byte => byte.toString(16).padStart(2, '0')).join('');
  const request = await createDeviceAuthorizationRequest(current, { applicationOrigin: location.origin, challenge, expiresAt: Date.now() + 120_000 });
  const pending: Pending = { request, returnPath: '/' + location.search + location.hash };
  sessionStorage.setItem(PENDING, JSON.stringify(pending));
  const target = new URL('/authorize', 'https://identity.meta-uber-engineer.dev');
  target.hash = new URLSearchParams({ 'device-request': encodeDeviceAuthorizationRequest(request) }).toString();
  location.assign(target.href);
}
