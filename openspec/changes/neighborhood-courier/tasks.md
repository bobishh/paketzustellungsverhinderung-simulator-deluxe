## 1. Workspace

- [ ] 1.1 Create the Cargo workspace with a Bevy game crate, a Bevy-free geometry crate, and a MetaMesh host crate
- [ ] 1.2 Add `vendor/meta-mesh` as a submodule at `ac782bf` and depend on `meta-mesh-core` and `meta-mesh-native`
- [ ] 1.3 Pin the current stable Bevy release and the Avian version that compiles with it
- [ ] 1.4 Add `THIRD_PARTY.md` with the TopoSaic revision, the Quaternius pack source, and the license notice for any copied source
- [ ] 1.5 Convert the Quaternius Public Transport bicycle to glTF and load it in Bevy

## 2. Neighborhood

- [ ] 2.1 Fetch and cache OSM tiles for an arbitrary origin, and check in one extract only as the test cache
- [ ] 2.2 Project WGS84 to local east-north meters around the session origin and test a known point without a window
- [ ] 2.3 Extrude each simple footprint into a building entity with OSM id, height rule, entrance, render mesh, and collider
- [ ] 2.4 Build street strips, sidewalks, and a ground surface per tile, and skip footprints that do not extrude
- [ ] 2.5 Start from a chosen coordinate and from a random occupied coordinate, load the next tile when the bicycle approaches it, and show the OpenStreetMap credit

## 3. Delivery

- [ ] 3.1 Simulate the imported bicycle with acceleration, steering, braking, and building collisions on a fixed timestep
- [ ] 3.2 Select one loaded building as the destination and mark it
- [ ] 3.3 Deliver only within 3 m of that entrance, and ignore a repeat confirmation

## 4. Shared session

- [ ] 4.1 Open a native MetaMesh workspace that records the session origin
- [ ] 4.2 Replicate the delivery by building OSM id and ignore a peer with a different origin
- [ ] 4.3 Replicate each bicycle pose and render the remote bicycle with the same model
- [ ] 4.4 Join a second process with an invitation and no separate game server

## 5. Verification

- [ ] 5.1 Run the geometry tests for projection, height fallback, broken footprints, and stable entrances
- [ ] 5.2 Run the delivery test for distance and repeat confirmation
- [ ] 5.3 Run the two-process session test for the shared delivery and the remote bicycle
