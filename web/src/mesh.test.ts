import { expect, it } from 'vitest';
import { decodeInvite, validFact, inviteLink } from './mesh';
const origin = { lat: 52.4, lon: 13.7 };
it('rejects unrelated world coordinates, spoofed actor devices and oversized facts', () => {
  const fact = { kind: 'actor', origin, device: 'alice', sequence: 2,
    pose: { avatar: 'courier', east: 1, north: 2, up: 0, heading: 0, velocity_east: 0, velocity_north: 0, parked_van: null } };
  expect(validFact(fact, origin, 'alice')).toBe(true);
  expect(validFact(fact, origin, 'bob')).toBe(false);
  expect(validFact({ ...fact, origin: { ...origin, lat: 0 } }, origin)).toBe(false);
  expect(validFact({ ...fact, origin: { ...origin, lat: NaN } }, origin)).toBe(false);
  expect(validFact({ ...fact, payload: 'x'.repeat(5000) }, origin)).toBe(false);
  expect(validFact({ ...fact, pose: null }, origin)).toBe(false);
  expect(validFact({ ...fact, pose: { ...fact.pose, east: NaN } }, origin)).toBe(false);
  expect(validFact({ ...fact, sequence: -1 }, origin)).toBe(false);
});
it('recognizes a browser invite and rejects incompatible native or malformed invites', () => {
  const invite = { protocol: 'paket-web/1', origin, host: 'a'.repeat(64), room: 'b'.repeat(64), relay: 'https://relay.example/' };
  expect(decodeInvite(JSON.stringify(invite))).toEqual(invite);
  for (const value of [{ ...invite, protocol: 'paket/1' }, { ...invite, relay: 'http://localhost/' }, { ...invite, room: 'guessable' }]) {
    expect(() => decodeInvite(JSON.stringify(value))).toThrow();
  }
});
it('bounds the shared clock and difficulty', () => {
  const fact={kind:'shift',origin,device:'host',elapsed_ms:1000,difficulty:1};
  expect(validFact(fact,origin,'host')).toBe(true);
  expect(validFact({...fact,day:2,elapsed_ms:0},origin)).toBe(true);
  expect(validFact({...fact,elapsed_ms:2880001},origin)).toBe(false);
  expect(validFact({...fact,difficulty:3},origin)).toBe(false);
  expect(validFact({...fact,elapsed_ms:-1},origin)).toBe(false);
});
it('validates day-scoped street actions while accepting legacy first-day peers',()=>{
  const fact={kind:'streetAction',origin,device:'dog',sequence:1,event:{action:'bite',target:'courier'}};
  expect(validFact(fact,origin)).toBe(true);
  expect(validFact({...fact,day:2},origin)).toBe(true);
  expect(validFact({...fact,day:0},origin)).toBe(false);
  expect(validFact({...fact,day:1.5},origin)).toBe(false);
});

it('accepts elevated and legacy parked vans but rejects invalid elevations', () => {
  const fact = { kind: 'actor', origin, device: 'alice', sequence: 2,
    pose: { avatar: 'courier', east: 1, north: 2, up: 0, heading: 0, velocity_east: 0, velocity_north: 0,
      parked_van: { east: 1, north: 2, heading: 0 } } };
  expect(validFact(fact, origin)).toBe(true);
  const atHeight = (up: number) => ({ ...fact, pose: { ...fact.pose, parked_van: { ...fact.pose.parked_van, up } } });
  expect(validFact(atHeight(2.345), origin)).toBe(true);
  expect(validFact(atHeight(NaN), origin)).toBe(false);
  expect(validFact(atHeight(Infinity), origin)).toBe(false);
});

it('shares the same URL after joining from a directory JSON invitation', () => {
  const invite = {protocol:'paket-web/1',origin,host:'a'.repeat(64),relay:'https://relay.example/',room:'b'.repeat(64)} as const;
  const listed = {room:invite.room,host:invite.host,protocol:invite.protocol,relay:invite.relay,origin:{lon:origin.lon,lat:origin.lat}};
  const link = inviteLink(invite,'https://game.test');
  expect(link.startsWith('https://game.test/#join=')).toBe(true);
  expect(inviteLink(decodeInvite(JSON.stringify(listed)),'https://game.test')).toBe(link);
  expect(decodeInvite(link)).toEqual(invite);
});
