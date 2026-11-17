## Why

Paket and Feed the Poor already render their worlds with Bevy, but their current browser interfaces and persistence do not ship with a native executable. Steam and phone releases need native menus, HUD, storage, lifecycle handling and usable controls. A Chromium or WebView wrapper is not the delivery strategy.

Repo City, called Vibe City in its native desktop build, already has a substantial native interface. Its mobile work is primarily navigation, readable code review and access to repository snapshots, rather than a courier-game UI port.

## What Changes

- Deliver Paket and Feed the Poor as native Bevy applications on desktop, Android and iOS. Replace their in-game browser dependencies with native UI and platform adapters; preserve existing game rules and progression.
- Introduce analog movement, camera and contextual actions that keyboard, mouse, controller and touch can drive. Keep the existing websites working during migration.
- Store native progress transactionally in OS application-data directories. Package assets without source-checkout paths. Retain existing save identities and signed Paket career rules.
- Make Paket's native and browser clients interoperable before advertising a full-parity public release. Preserve host authority, invitations and delivery receipts.
- Add a mobile-web route using the existing WASM renderer and touch controls. Publish a measured support matrix instead of promising every phone can run WebGPU.
- Plan Repo City's phone/tablet experience around GitHub snapshots and review. Keep desktop local repositories, watchers and MCP as desktop capabilities.
- Define implementation stages, acceptance gates, effort ranges and independent store lead times. This change specifies future work; it does not implement ports, register accounts or publish applications.

## Capabilities

### New Capabilities

- `native-game-client`: Native game UI, durable storage, map services, session parity and application lifecycle.
- `portable-player-input`: Analog and contextual controls across desktop, controller and touch.
- `platform-distribution`: Tested native packages, mobile-web compatibility and store release gates.
- `repo-city-mobile-review`: Touch navigation and snapshot review for Repo City/Vibe City.

### Modified Capabilities

None. The existing `shared-street` change describes an earlier native prototype and is not the current browser session contract. The new requirements explicitly preserve the current authoritative delivery and career behavior.

## Impact

- Paket: `crates/game`, `crates/session`, browser command/state adapters and release packaging.
- Feed the Poor: `crates/game`, `web/src/campaign.ts`, HUD/report adapters and release packaging in its own repository.
- Repo City/Vibe City: `crates/explorer`, repository-source adapters and native review presentation in its own repository.
- This directory is the canonical cross-project plan. Implementation changes belong to their owning repositories; it does not create another application, backend or shared framework.

See [design and estimates](design.md), [implementation tasks](tasks.md) and the [existing Steam guide](../../../docs/steam.md).
