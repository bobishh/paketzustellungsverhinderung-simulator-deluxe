## Why

A shared game needs a real street, not a made-up city and not a page inside the blog. The street is whichever place the session starts in. OpenStreetMap already has the footprints. TopoSaic already turns those footprints into geometry. MetaMesh already syncs a signed workspace between native peers.

## What Changes

- Start a standalone native game, Paketzustellungsverhinderung Simulator Deluxe, in this repository. It is not embedded in an existing site, and it does not add a second web app.
- Start a session at a chosen coordinate or at a random place that contains streets and buildings. That coordinate is the origin. Tiles of OSM geometry load around the bicycle and further tiles load as it rides. There is no fixed neighborhood size.
- Keep each building as its own object with a stable OSM id. Streets, sidewalks, and ground are separate from buildings.
- Let the player ride a bicycle, collide with buildings, reach one destination entrance, and deliver a parcel. The bicycle is an existing CC0 model, not geometry authored for this game.
- Record the session origin, the delivery, and the bicycle pose in a MetaMesh workspace. A second local peer that joins the same origin sees the same buildings, the delivered parcel, and the other bicycle.
- Treat TopoSaic as a source of OSM, projection, and extrusion algorithms under Apache-2.0. Do not import its print pipeline, HTTP service, or React preview.

## Capabilities

### New Capabilities

- `neighborhood-scene`: A chosen or random map origin becomes a rideable Bevy zone of identifiable buildings, streets, and colliders, and the zone grows by tiles as the bicycle moves.
- `bicycle-delivery`: A player rides a bicycle to one real entrance and delivers a parcel there.
- `shared-street`: Two native peers share that origin, the delivery, and bicycle poses through MetaMesh.

### Modified Capabilities

## Impact

- New Cargo workspace: a Bevy game, a geometry crate, and a host crate over `meta-mesh-core` and `meta-mesh-native`.
- `vendor/meta-mesh` is a git submodule pinned to the revision tincanban already uses (`ac782bf`). The browser crate `meta-mesh` is not a dependency.
- The bicycle mesh comes from the Quaternius Public Transport Pack (https://quaternius.com/packs/publictransport.html), converted once to glTF. The pack is free for commercial use.
- TopoSaic (https://github.com/theatrus/toposaic) is studied and, where copied, attributed. Its application is not vendored.
- A checked-in OSM extract exists only so tests and an offline launch have a deterministic cache. Publishing a derived geographic database is a separate ODbL decision and is not part of this change.
