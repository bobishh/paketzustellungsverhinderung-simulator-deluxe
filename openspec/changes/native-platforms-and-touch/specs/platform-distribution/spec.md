## ADDED Requirements

### Requirement: Application packages use native rendering
Desktop, Android and iOS application releases SHALL package native Bevy execution and native game UI. They MUST NOT depend on Electron, CEF, Tauri, Capacitor or another browser wrapper for the game. Platform features and dependency gates SHALL be verified on the pinned engine version before claiming support.

#### Scenario: Clean native install launches independently
- **GIVEN** the declared target device has no source checkout, developer tools or game website server
- **WHEN** the player installs and starts the release candidate
- **THEN** the packaged assets, native UI and native graphics initialize correctly

### Requirement: Mobile web declares measured compatibility
The mobile-web release SHALL retain the WASM game with touch input and check required GPU capabilities before loading its world. Supported browser/OS/GPU combinations SHALL be based on real-device verification. WebGL2 support SHALL remain a separate renderer decision until proven.

#### Scenario: Browser lacks the required GPU capability
- **GIVEN** the player's mobile browser cannot provide the required WebGPU device
- **WHEN** they attempt to start
- **THEN** the interface explains compatibility and a supported alternative before an indefinite map-loading flow

#### Scenario: Supported mobile browser starts a game
- **GIVEN** a declared supported phone/browser and a valid selected location
- **WHEN** the player starts and uses touch input
- **THEN** the world, HUD and contextual controls work through the real WASM route

### Requirement: Performance qualification includes sustained device use
Each advertised platform SHALL have named test devices, representative scenes and measured startup, memory and frame-time results. The initial targets SHALL be stable 60 FPS on the declared desktop test machine and at least 30 FPS on declared mobile devices during representative 20-minute play, with mobile p95 frame time no greater than 50 ms. UI SHALL stay sharp independently of scene resolution. Unmet targets SHALL lead to optimization or a narrower declared support matrix, not unsupported claims.

#### Scenario: A phone slows after warming up
- **GIVEN** a supported phone runs the representative scene for twenty minutes
- **WHEN** sustained frame-time or memory behavior fails the declared target
- **THEN** the candidate fails qualification for that device until optimized or the support declaration is revised

### Requirement: Store releases follow verified platform gates
Release candidates SHALL preserve saves across updates, include attribution/licenses and accurately describe tested capabilities. Store submissions SHALL use reproducible artifacts and required ratings, privacy and generated-content disclosures. Engineering estimates SHALL exclude external account verification, mandated test periods and review wait times. Steam software eligibility SHALL be confirmed separately for Repo City.

#### Scenario: An untested platform is absent from store claims
- **GIVEN** only the Windows native candidate passed package acceptance
- **WHEN** the Steam store page is prepared
- **THEN** it does not advertise macOS, Linux, Steam Deck or browser cross-play without their required acceptance evidence

#### Scenario: Production access requires a closed test
- **GIVEN** a Google Play account is subject to the new-personal-account testing requirement
- **WHEN** its application is prepared for production
- **THEN** the required closed testing and access application are completed before public publication
- **AND** that calendar wait is not represented as completed engineering work
