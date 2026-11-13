## Context

The game is a new native program. Players eventually share a real place for delivery, racing, or fighting. This change builds the street those modes stand on: a session can start in any district, one bicycle, one parcel, and a second peer who can see that happen.

An earlier note used 500 × 500 m of Berlin as a prototype sample. That size is not a rule of the game. Baking it into the session would force a rewrite the moment the bicycle leaves the sample. The session origin is a coordinate. The loaded area is whatever tiles the ride has reached.

TopoSaic already loads OSM, projects WGS84 into a local frame, extrudes buildings, and mixes terrain heights. It does that to export a printable solid. A printable solid is one mesh. A game needs one object per building, a collider that is not the render mesh, and an id that survives a reload. The useful files are `osm.rs`, `geography.rs`, `mesh.rs`, and the geometry portion of `preview.rs`. `preview.tsx` and the print API are not useful here.

MetaMesh already has a native runtime. `meta-mesh-core` owns identity, signed records, and Automerge sync. `meta-mesh-native` owns the Iroh endpoint, gossip, and blob transfer. The game is another host of that runtime, the same way tincanban is a host of the workspace. The game does not reimplement pairing, signatures, or replication.

## Goals / Non-Goals

**Goals:**

- A `cargo run` window starts at a chosen coordinate or at a random coordinate that has streets and buildings.
- Riding toward an unloaded area fetches and spawns the next tile. Leaving a tile does not end the session.
- Each building is its own entity, with its OSM id, footprint, height, and entrance.
- A bicycle accelerates, steers, brakes, and collides. Riding to the selected entrance and confirming delivers the parcel.
- Two processes with the same origin see the same building ids, the delivery, and each other's bicycle.
- Geometry is rebuilt from OSM tiles. Peers do not send meshes to each other.

**Non-Goals:**

- A maximum ride distance, a match box, or a Berlin-only map.
- Embedding the game in the blog or the job-hunting page.
- Three.js, Rapier-as-the-renderer, or a browser build. A later WASM build can exist; this slice does not target it.
- Authoring a bicycle mesh, a rider, or a city kit.
- Aliens, weapons, destruction, racing rules, mission generation, and courier career progression.
- Forking the TopoSaic application or depending on its server.
- A dedicated game server. MetaMesh is the session.
- Publishing or redistributing a derived OSM database.
- Terrain meshes, bridges, and indoor floor plans. The ground is a flat local surface. Heights come from OSM tags or the fallback below.

## Decisions

### One Bevy app, one Rust workspace

The window, schedule, and input live in Bevy. Geographical processing lives in a library crate with no Bevy types, so coordinate and footprint tests do not open a window. The MetaMesh host is a third crate that owns the native runtime and exposes workspace facts to Bevy as components. The browser WASM adapter is not linked.

Bevy is the current stable release at implementation time. Physics is Avian, stepped on a fixed timestep. Avian stays inside the Bevy schedule. Rapier through WASM was the alternative for a page embedded in the site; this program is not that page.

### The origin is a coordinate, tiles are only a fetch unit

The player enters a latitude and longitude, or the game draws a random coordinate and accepts it when the first tile contains at least one building and one street. That coordinate is the session origin for the whole ride. Projection converts WGS84 to meters east and north of the origin, Y up. Gameplay, colliders, and replicated poses use those meters.

OSM is requested per tile and cached on disk. A tile is how much map is fetched and spawned at once. Its edge is not the edge of the world. When the bicycle comes within a margin of a missing neighbor, that tile is loaded. Tiles far from the bicycle are despawned. Reloading a tile restores the same OSM ids and the same meter positions. The tile span used by the fetcher is an implementation parameter, not a number in the rules.

A checked-in extract is a pre-seeded cache for tests and for an offline launch of that one origin. It is not the map the game ships as its only place.

### Buildings are objects, print meshes are not

A building footprint becomes one entity. The render mesh is an extrusion of that footprint. The collider is a simpler solid of the same footprint, not a triangle soup taken from a print mesh. Height is `height` when the tag parses as meters, otherwise `building:levels` times 3 m, otherwise 9 m. An entrance is an `entrance` node on the footprint when the extract has one, otherwise the footprint point closest to a street.

Streets are strips from way centerlines, with a sidewalk offset. They do not become buildings. Each loaded tile has a ground surface at y = 0.

### The bicycle is an imported model

The visible bicycle is the bicycle from the Quaternius Public Transport Pack (February 2017, https://quaternius.com/packs/publictransport.html), converted once from the pack's FBX or OBJ into glTF. Bevy loads that glTF. The pack is free for personal and commercial projects. The physics body is a box or capsule fitted to the model, not the model's triangles, and not a bicycle built from Bevy primitives. Wheel spin can follow the model's wheel nodes when those nodes exist. No new bicycle mesh is modeled for this slice.

### TopoSaic is a donor, not a dependency

Read `osm.rs`, `geography.rs`, `mesh.rs`, and the geometry construction in `preview.rs`. Reimplement the parts that fit the crates above. Copy a function only when the adaptation would be a rename, and keep its Apache-2.0 header. Do not copy the Three.js preview, the print exporter, or the service. Record the source revision that was read in `THIRD_PARTY.md`.

### The mesh shares the origin, not triangles

A session document names the origin. A delivery document names the building OSM id, the outcome, and the signing device. A pose document names the device and its meter position and heading relative to that origin. All three are ordinary workspace records replicated by the existing native runtime. A peer whose origin differs does not apply the other peer's poses or deliveries.

Both peers generate a tile from the same OSM tile key. They do not wait for a single pre-baked extract. The second peer is started as a second process with an invitation from the first. Seeing the other bicycle and the delivered building is the multiplayer proof. Interest management, authority over who may deliver, and anti-cheat are later.

### OSM credit stays on screen

The window shows "© OpenStreetMap contributors". Cached tiles record the fetch time and the source. The game does not offer those tiles as a geographic database for other products.

## Risks / Trade-offs

- [A random coordinate lands in the ocean or an empty tile.] → Draw another point until the first tile has a building and a street.
- [A copied TopoSaic function pulls in print-only assumptions.] → Copy only after the unit test for that function is written against the test extract. Otherwise rewrite it.
- [OSM footprints are invalid polygons.] → Drop a footprint that does not produce a simple extrusion and count it. One missing shed does not fail the tile.
- [The native MetaMesh API is larger than a game session needs.] → Use the existing workspace and document sync. Do not add a game-specific transport.
- [Two processes generate different meshes from the same tags.] → Tests compare entity ids and entrance meters, not vertex buffers.
- [ODbL share-alike applies if a derived database is shipped.] → This change caches tiles for play and tests. Anything broader waits for a separate decision.
- [The Quaternius bicycle has no separate wheel nodes.] → The whole model yaws and moves. Wheel spin waits until the converted glTF has those nodes.
- [Avian and the pinned Bevy release may not match on the day of implementation.] → Pin the pair that compiles together and record both versions. Do not switch the renderer to Three.js to avoid that pin.

## Migration Plan

This is the first commit of the game repository. There is no previous session format to migrate. A session stores an origin. A peer with a different origin does not reuse the first peer's poses or deliveries.

## Open Questions

None for this slice. The test extract's origin is whatever coordinate its manifest records. That manifest is test data, not the list of places the game allows.

## Map selector and streamed rides

Paket opens on a pan-and-zoom OpenStreetMap map. The crosshair selects the spawn area. New Ride starts a session at that point and projects the spawn to the nearest street segment. The selected origin fixes the local coordinate frame for every peer, and riding can continue beyond the initial area. Joining a pasted invite starts at its origin. The ride screen copies the invite link.

The engine fetches OSM geometry around the bike and prioritizes tiles in the direction of travel. A nearby safety margin also loads collision geometry beside and behind the player. A wider retention radius keeps nearby entities alive across tile boundaries. The bike waits at the boundary if the next tile has not loaded. Feature IDs shared by overlapping tiles reference the same scene entity, which unloads after the last tile that references it leaves range.

An invitation carries the WGS84 origin, room identifier, and Iroh endpoint addresses. Separate hosts at the same origin get separate rooms. Peers share local east, north, and up positions, heading, horizontal velocity, and snapshot sequence. Remote bikes blend toward the latest snapshot, briefly extrapolate through gaps, and disappear after a timeout. The game builds its OSM scene at runtime.
