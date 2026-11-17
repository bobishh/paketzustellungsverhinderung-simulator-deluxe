## ADDED Requirements

### Requirement: Native applications render the complete game interface
Paket and Feed the Poor native releases SHALL render their game menus, HUD, interactions, settings and reports with Bevy UI. They MUST run without an embedded browser or a local website server. The existing Rust game rules SHALL remain authoritative for gameplay, economy and progression.

#### Scenario: A fresh native installation completes a game flow
- **GIVEN** the player installed the native package on a supported device without the source checkout
- **WHEN** they select a location and character or role, start play, interact and finish or leave the run
- **THEN** the native interface provides the required controls, loading states, HUD and persistent result view
- **AND** no browser document is required to perform a game action

#### Scenario: Map loading fails
- **GIVEN** a selected area has no usable cached geometry and its map request fails
- **WHEN** the request reaches its bounded failure condition
- **THEN** the application shows a centered recoverable error with retry and location selection
- **AND** it does not remain indefinitely in a blank scene or spinner

### Requirement: Native and browser interfaces invoke the same rules
Each interface SHALL submit typed game actions to the owning Rust behavior and display its state projection. Browser serialization SHALL remain at the WASM boundary. Porting MUST NOT create a second delivery ledger or economy, remove existing mechanics or shorten Feed the Poor's long-term progression.

#### Scenario: Equivalent ATM actions produce equivalent state
- **GIVEN** native and browser test clients start from equivalent valid campaign snapshots
- **WHEN** each completes the same permitted ATM withdrawal
- **THEN** the same game rule updates cash, available ATM funds, spending and fortune consistently

#### Scenario: Existing browser play remains available during migration
- **GIVEN** a native UI increment has been added
- **WHEN** the current browser build is launched
- **THEN** its existing game actions and saves continue to work through the browser adapter

### Requirement: Native data survives restarts and package updates
One persistence owner SHALL store versioned native progress transactionally in OS application-data locations. Assets SHALL load from the shipped package and writable cache/photos from OS writable locations. Save migration or import MUST preserve domain IDs, original fortune, cumulative spending, charity buildings and valid signed career receipts. Invalid imports MUST leave existing data intact. Device keys MUST NOT be copied implicitly.

#### Scenario: Restart after a package update
- **GIVEN** a valid campaign or career was saved by the previous supported native version
- **WHEN** the updated package starts from a different working directory with a read-only installation directory
- **THEN** it restores the progress and settings without requiring the developer's checkout

#### Scenario: Large charity collection remains intact
- **GIVEN** a campaign contains 333,000 charity-building records
- **WHEN** one record changes and the application saves and restarts
- **THEN** all valid records and money invariants survive
- **AND** saving does not rewrite or project the entire collection every render frame

#### Scenario: Invalid imported campaign is rejected
- **GIVEN** a native profile already has valid saved progress
- **WHEN** a player imports an unsupported or invalid campaign file
- **THEN** the application explains the rejection and retains the previous valid progress

### Requirement: Paket native and browser sessions preserve shared authority
Public full-parity Paket releases SHALL support the same versioned application session contract on native and browser transports. The host SHALL own route, clock and first completion per address/day. Credits SHALL remain assigned to the receipt's delivering player. Signed careers SHALL retain existing verification and merge rules; a Steam account MUST NOT silently replace Identity.

#### Scenario: Native player joins a browser-hosted mesh late
- **GIVEN** a browser host has a route with completed deliveries and an advanced shift clock
- **WHEN** a native player follows its valid invitation
- **THEN** the native player receives the same route, clock, players and completion receipts
- **AND** the player does not receive somebody else's delivery credit

#### Scenario: Browser player joins a native host and replays a completion
- **GIVEN** a native host has already accepted a receipt for an address and day
- **WHEN** a browser client joins and submits that completion again
- **THEN** the host keeps the original completion and does not award duplicate credit

#### Scenario: Host restarts with a saved mesh
- **GIVEN** the host has persisted its mesh identity, invitation and authoritative state
- **WHEN** it restarts and becomes reachable again
- **THEN** returning players can reconnect to that mesh and recover its consistent state

#### Scenario: Invitation protocol cannot be used
- **GIVEN** an invitation names an unsupported session version
- **WHEN** a native or browser player attempts to join
- **THEN** the interface explains the incompatibility and an actionable recovery
- **AND** it does not wait indefinitely or discard the current save

### Requirement: Lifecycle events preserve play and recover resources
Native applications SHALL clear held input, preserve durable progress and recover interrupted rendering/network resources on supported lifecycle transitions. Feed the Poor solo play SHALL pause while backgrounded. Paket SHALL reconcile its authoritative session on resume and MUST NOT assume a suspended mobile host remains reachable.

#### Scenario: Feed resumes after backgrounding
- **GIVEN** the player is in an active Feed the Poor run
- **WHEN** the app is backgrounded and later foregrounded
- **THEN** the run restores without a fresh campaign, stuck input or simulated background attacks

#### Scenario: A Paket mobile host is suspended
- **GIVEN** peers are connected to a mobile host
- **WHEN** the OS suspends that host
- **THEN** clients show the unavailable connection and preserve their recoverable state
- **AND** the host reconciles saved state when it resumes instead of creating a new shift silently
