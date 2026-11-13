## ADDED Requirements

### Requirement: A session starts at a map origin
The game SHALL start a session at a latitude and longitude supplied by the player, or at a random coordinate. A random coordinate MUST be rejected and redrawn until its first tile contains at least one building and one street. That accepted coordinate MUST be the projection origin for the rest of the session. The playable area MUST NOT be limited to a fixed neighborhood size.

#### Scenario: Chosen coordinate
- **WHEN** the player starts a session at a coordinate whose first tile contains buildings and streets
- **THEN** the scene is projected around that coordinate and contains those buildings and streets

#### Scenario: Empty random coordinate
- **WHEN** a random coordinate falls in a tile with no building or no street
- **THEN** another coordinate is drawn and the empty tile does not become the session

### Requirement: Riding loads the next tile
OSM data SHALL be fetched and cached per tile. When the bicycle approaches a missing neighboring tile, the game MUST load that tile and continue the same session. Despawning a distant tile MUST NOT forget its OSM ids. Reloading that tile MUST restore the same ids and the same meter positions. A checked-in extract MAY seed the cache for tests and for an offline launch of that extract's origin. A normal networked launch MUST NOT require that extract.

#### Scenario: Ride past the first tile
- **WHEN** the bicycle approaches an unloaded neighboring tile
- **THEN** that tile's buildings and streets appear and the session origin stays unchanged

#### Scenario: Return to a despawned tile
- **WHEN** a tile is despawned and later loaded again
- **THEN** each of its building OSM ids appears once at its previous meter position

#### Scenario: Offline seeded origin
- **WHEN** the game starts offline on the checked-in extract's origin
- **THEN** the scene is built from that cache and no map request is made

### Requirement: Buildings stay separate objects
Each building footprint SHALL be one entity. The entity MUST keep its OSM id, its projected footprint, its height, and its entrance. The render mesh MUST be an extrusion of that footprint. The collider MUST be a separate solid of the same footprint. Height MUST use the `height` tag when it parses as meters, otherwise `building:levels` times 3 m, otherwise 9 m. An entrance MUST be an `entrance` node on the footprint when the tile contains one, otherwise the footprint point closest to a street.

#### Scenario: Tagged height
- **WHEN** a footprint has `height` of 12 m
- **THEN** that building's extrusion is 12 m and its collider covers the same footprint

#### Scenario: Level fallback
- **WHEN** a footprint has no parseable `height` and has `building:levels` of 4
- **THEN** that building's extrusion is 12 m

#### Scenario: Default height
- **WHEN** a footprint has neither a parseable `height` nor `building:levels`
- **THEN** that building's extrusion is 9 m

#### Scenario: Broken footprint
- **WHEN** a footprint cannot be extruded as a simple polygon
- **THEN** that footprint is omitted, the omission is counted, and the remaining buildings still load

### Requirement: Streets and ground are not buildings
Street centerlines SHALL become street strips with a sidewalk offset. Each loaded tile SHALL have a ground surface at y = 0. A street MUST NOT receive a building OSM id.

#### Scenario: Road way
- **WHEN** a tile contains a road way
- **THEN** the scene contains a street strip for that way and no building entity uses that way's id

### Requirement: The window credits OpenStreetMap
The running game MUST show "© OpenStreetMap contributors". Copied TopoSaic source MUST keep its Apache-2.0 notice, and `THIRD_PARTY.md` MUST name the TopoSaic revision that was read.

#### Scenario: Visible credit
- **WHEN** the neighborhood window is open
- **THEN** the OpenStreetMap credit is visible

### Requirement: Projection is tested without a window
Coordinate conversion and footprint extrusion SHALL be callable without creating a Bevy window. A known WGS84 point in the test extract MUST project to a stable east-north meter position relative to that extract's origin.

#### Scenario: Known point
- **WHEN** the test origin and a second known point are projected
- **THEN** the origin is at the local origin and the second point matches the expected meters within 0.1 m
