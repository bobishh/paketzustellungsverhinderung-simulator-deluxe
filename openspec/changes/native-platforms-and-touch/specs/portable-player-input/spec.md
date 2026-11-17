## ADDED Requirements

### Requirement: Devices drive one analog action model
Keyboard/mouse, controller and touch SHALL drive the same Rust movement, look and contextual actions. The model SHALL carry analog axes and action press/release state. Touch look MUST work without browser Pointer Lock. Losing a touch, focus, controller or active app state MUST clear affected held actions.

#### Scenario: Move, aim and act together
- **GIVEN** the player uses touch controls on a supported native or web device
- **WHEN** one touch moves, a second drags the camera and another activates an action
- **THEN** movement, camera and the action are tracked independently with no simulated keyboard requirement

#### Scenario: Touch is canceled during movement
- **GIVEN** a touch is holding movement or a repeated action
- **WHEN** the OS or browser cancels that touch or backgrounds the app
- **THEN** the held movement or action stops and remains stopped until new input

### Requirement: Contextual controls retain every gameplay mode
Touch interfaces SHALL provide movement/look, contextual interaction and a discoverable tool/action selector. They SHALL expose the game's existing modes without requiring every desktop hotkey to remain visible simultaneously. Sensitivity and left-handed settings SHALL be available. Controller users SHALL be able to navigate the same game flows and menus.

#### Scenario: Paket player returns the wrong parcel using touch
- **GIVEN** the player holds a parcel beside their van
- **WHEN** they choose return or exchange through the contextual touch controls
- **THEN** the existing parcel rule performs the requested action without requiring an intercom or keyboard

#### Scenario: Paket changes interaction mode
- **GIVEN** the player uses touch only
- **WHEN** they drive, exit, collect a parcel, ring an intercom, choose a dialogue answer and take a photo
- **THEN** each mode presents usable controls and invokes the existing gameplay action

#### Scenario: Feed player accesses late progression tools
- **GIVEN** a valid campaign has unlocked shops, abilities, vehicles and charity actions
- **WHEN** the touch-only player switches tools and opens those interactions
- **THEN** all unlocked actions remain accessible without simplifying the economy or progression

### Requirement: UI focus prevents gameplay input leakage
Menus, text fields and dialogs SHALL consume their input before gameplay. Opening or closing a modal SHALL clear conflicting actions. Native text entry SHALL support Unicode. UI sizing SHALL respect safe areas and remain independent of adaptive 3D resolution.

#### Scenario: Closing a shop does not throw money
- **GIVEN** a player is interacting with a shop overlay
- **WHEN** they tap its close control or return focus to the scene
- **THEN** that input does not fire, move or trigger a game interaction

#### Scenario: Scene quality changes on a phone
- **GIVEN** a phone uses adaptive scene quality and a display cutout
- **WHEN** the application reduces scene resolution
- **THEN** HUD text and controls keep their display-scale size and sharpness inside the safe area
- **AND** the aiming and camera regions remain usable
