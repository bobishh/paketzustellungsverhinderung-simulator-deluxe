import { decodeInvite } from './mesh';

export function publishMesh(invite: unknown, token: string) {
  let closed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let notified = false;
  const send = async (method: 'PUT' | 'DELETE') => {
    const response = await fetch('/api/meshes', {method,headers:{authorization:`Bearer ${token}`,'content-type':'application/json'},body:JSON.stringify(invite),signal:AbortSignal.timeout(8000)});
    if (!response.ok) throw new Error('Mesh directory unavailable');
  };
  const renew = async () => {
    try { await send('PUT'); notified = false; }
    catch {
      if (!closed && !notified) paketNotify('Could not list your mesh. Share its invitation directly; listing will retry.');
      notified = true;
    }
    if (!closed) timer = setTimeout(renew,30_000);
    else { try { await send('DELETE'); } catch { /* The lease expires without renewal. */ } }
  };
  void renew();
  return () => { closed = true; clearTimeout(timer); void send('DELETE').catch(()=>undefined); };
}

export async function availableMeshes() {
  const response = await fetch('/api/meshes',{cache:'no-store',signal:AbortSignal.timeout(8000)});
  if (!response.ok) throw new Error('Mesh list unavailable. Refresh or paste an invitation.');
  const data: unknown = await response.json();
  if (!Array.isArray(data) || data.length > 256) throw new Error('Invalid mesh list');
  return data.flatMap(value => { try { return [decodeInvite(JSON.stringify(value))]; } catch { return []; } });
}
