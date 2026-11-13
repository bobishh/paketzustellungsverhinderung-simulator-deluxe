## ADDED Requirements

### Requirement: The bicycle moves and collides
The player SHALL control one bicycle whose visible mesh is the bicycle from the Quaternius Public Transport Pack, converted to glTF. The game MUST NOT author a replacement bicycle mesh. The physics body MUST be a box or capsule fitted to that model. Control MUST include acceleration, steering, and braking. The bicycle MUST collide with building colliders and MUST NOT pass through them. Simulation MUST advance on a fixed timestep.

#### Scenario: Ride into a building
- **WHEN** the player accelerates the bicycle into a building collider
- **THEN** the bicycle stops at the collider and the building entity remains

#### Scenario: Steer and brake
- **WHEN** the player steers while moving and then brakes
- **THEN** the heading changes and the bicycle slows to a stop

### Requirement: One building is the delivery target
The session SHALL select one building from the loaded neighborhood as the parcel destination. The selected building MUST be visibly distinct. The destination MUST be that building's entrance.

#### Scenario: Target is a real building
- **WHEN** a neighborhood has loaded
- **THEN** exactly one building is the destination and its OSM id is the delivery target

### Requirement: Delivery happens at the entrance
The player MUST be able to deliver the parcel only when the bicycle is within 3 m of the destination entrance. A successful delivery MUST record the building OSM id and the delivered outcome. A second confirmation MUST NOT create a second delivery for that building.

#### Scenario: Too far
- **WHEN** the player confirms delivery more than 3 m from the destination entrance
- **THEN** the parcel stays undelivered

#### Scenario: At the entrance
- **WHEN** the bicycle is within 3 m of the destination entrance and the player confirms delivery
- **THEN** that building OSM id is recorded as delivered

#### Scenario: Repeat confirmation
- **WHEN** the player confirms delivery again for the same building
- **THEN** the recorded delivery is unchanged
