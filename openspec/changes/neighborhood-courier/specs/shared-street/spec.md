## ADDED Requirements

### Requirement: A session names the origin
The first process SHALL open a MetaMesh workspace through `meta-mesh-native` and record the session origin. A peer MUST NOT apply deliveries or poses from a session with a different origin. Peers with the same origin MUST build a given tile from the same OSM tile key and MUST NOT require one pre-baked extract of the whole ride.

#### Scenario: Matching origin
- **WHEN** a second process joins the workspace at the same origin
- **THEN** it loads the same building OSM ids for each tile it opens

#### Scenario: Different origin
- **WHEN** a peer with a different origin connects
- **THEN** it does not apply the session's deliveries or bicycle poses

### Requirement: The delivery replicates
A delivery recorded by one peer SHALL appear on the other peer as the same building OSM id and delivered outcome. The peers MUST NOT transfer render meshes or colliders.

#### Scenario: Second process sees the parcel
- **WHEN** the first process delivers the parcel and the second process has joined the same workspace
- **THEN** the second process shows that building OSM id as delivered

### Requirement: The other bicycle is visible
Each peer SHALL replicate its bicycle position and heading in local meters. The other peer MUST show that bicycle as a separate model at the replicated pose. A peer MUST keep simulating its own bicycle locally between updates.

#### Scenario: Two bicycles
- **WHEN** both processes have joined and one bicycle moves
- **THEN** the other process shows two bicycles and the remote bicycle follows the replicated pose

### Requirement: The native runtime is the session
The game MUST link `meta-mesh-core` and `meta-mesh-native` from the pinned `vendor/meta-mesh` submodule. It MUST NOT depend on the browser `meta-mesh` crate or on a separate game server.

#### Scenario: Two local processes
- **WHEN** the second process is started with an invitation from the first and no other server is running
- **THEN** the delivery and the remote bicycle still appear
