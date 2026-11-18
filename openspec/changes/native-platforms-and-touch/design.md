## Context

Planning baseline: 17 November 2026. Effort ranges below are engineering estimates, not completed-port claims or store guarantees.

### Current code

| Project | Reusable implementation | Work still required |
| --- | --- | --- |
| Paket | Bevy 0.20 world, physics, map geometry, courier rules and native MetaMesh runtime. | Current lobby, HUD, Identity and career experience lives largely in `web/src`. Native and browser session protocols differ. Native assets, cache and photos resolve through `CARGO_MANIFEST_DIR`. |
| Feed the Poor | Bevy world and Rust economy, encounters and progression. | Native `platform::ui_event` does nothing and `ui_commands` returns an empty array. Current HUD, reports and browser campaign storage need native implementations. Assets/cache use checkout paths. |
| Repo City / Vibe City | Native Bevy interface, VibePad, SQLite review state, GitHub device sign-in and a macOS bundle script. Browser already reads GitHub snapshots. | Touch navigation, phone-readable review, mobile source/auth/storage adapters and production packaging. The macOS script currently packages a development build. |

Evidence locations: Paket [build features](../../../crates/game/Cargo.toml), [session dependencies](../../../crates/session/Cargo.toml), [application paths](../../../crates/game/src/main.rs) and [README](../../../README.md); Feed the Poor [native bridge](https://github.com/bobishh/feed-the-poor/blob/a436e10b5a7534ae2430591fd0757bc5f81e2387/crates/game/src/platform.rs) and [campaign persistence](https://github.com/bobishh/feed-the-poor/blob/a436e10b5a7534ae2430591fd0757bc5f81e2387/web/src/campaign.ts); Vibe City [native features](https://github.com/bobishh/vibe-city/blob/adf500518671abd99bfc9961b72b528a696fd206/README.md) and [active package](https://github.com/bobishh/vibe-city/blob/adf500518671abd99bfc9961b72b528a696fd206/crates/explorer/Cargo.toml). A successful desktop compile alone does not establish feature parity.

## Goals and exclusions

The application releases run Rust/Bevy directly, with a native window/surface and game UI. They do not require an installed browser, local website server or embedded Chromium/WebView. Keep the engine and game content; do not rebuild the games in another engine.

Steam starts with a Windows x64 candidate. macOS, Linux and Steam Deck support require separate package and device verification. Android starts with arm64; iOS covers iPhone and iPad. Mobile web remains WASM and is a separate delivery target.

Preserve Paket deliveries, dogs, driving, photos and career semantics. Preserve Feed the Poor's full existing progression, long-term fortune scale, cash/food, ATMs, bosses, abilities, vehicles, shops and charity buildings. Phone controls reorganize access to those mechanics; they do not remove them or shorten the economy.

Do not redesign the public landing pages, change the cast or add monetization in this port. Store accounts, fees, credentials and publication are separate release operations. No mobile local-checkout or desktop-MCP promise for Repo City.

## Decisions

### 1. Keep five concepts in the client boundary

| Concept | Owner and invariant |
| --- | --- |
| Game state | Existing Rust game rules own money, deliveries, encounters and progression. UI never owns a second economy or delivery ledger. |
| Player input | One Rust action resource carries analog movement/look and contextual actions into existing rules. |
| Game UI | The same Bevy UI presents state and submits game actions on native desktop, phones and WASM. Browser code supplies platform capabilities rather than a second renderer. |
| Persistence | One storage module owns versioned snapshots, atomic writes, migrations and restore validation. |
| Platform services | A narrow adapter provides paths, network access, credentials, clipboard/share and lifecycle notifications. |

The normal flow is UI/input → game rules → persistence or platform service. Keep game-specific widgets and rules in their owning game. Share small controls or adapters only after both games demonstrate the same contract; do not introduce a general UI engine, another service or a repository merger.

### 2. Replace browser dependencies at the game boundary

Use Bevy UI for native launch/location selection, role/character selection, loading/error states, HUD, settings/help, interaction dialogs, inventory/tool selection, pause/quit and end-of-run reports. Paket additionally needs mesh discovery/invitations/share, roster/minimap, profile and Identity approval. Feed additionally needs ATM/PIN/call flows, shops, progression purchases, charity management and its persistent newspaper report.

Read the current browser command/state contracts and replace stringly native no-ops with typed Rust actions and state projections. Keep WASM serialization at the browser boundary. Define actions alongside the owning game behavior, not in a new central registry of every subsystem. Both front ends must exercise the same Rust transition.

Compile the shared Bevy widgets to WASM in both games. Keep browser HTML limited to the canvas and technical bootstrap/compatibility failures; retain a hidden OS-keyboard input when required by mobile browser activation rules. Preserve existing generated artwork, game actions, IndexedDB saves and multiplayer authority. Remove obsolete DOM widget renderers after their real-game acceptance flows have been adapted. OS text entry, Unicode names/addresses, scaling, safe areas and focus handling remain required.

Render the street through a separate adaptive texture and composite it beneath the full-resolution UI camera. Camera rays and world-label projections use logical window coordinates at every scene scale. Publish a bounded read-only projection of actual laid-out Bevy widgets for browser pointer-lock routing and isolated acceptance tests; it must not become a second UI or game-state owner.

### 3. Use native persistence and explicit data movement

Use SQLite for native campaign/career records and large charity-building collections, with transactional migrations and bounded incremental updates. Do not serialize or redraw hundreds of thousands of centers every frame. Use OS application-data/cache locations and packaged read-only assets; never depend on the source checkout or current working directory. Keep private credentials in the platform credential store where supported.

Preserve existing domain IDs, integer money units, original fortune, cumulative spending, save versions and signed career receipts. Repository snapshots/review drafts remain owned by Repo City's existing storage, independent of its world presentation.

Browser IndexedDB does not automatically become accessible to a native app. Add an explicit versioned export/import path for portable campaign/settings data, with schema validation and transactional rollback. Do not silently copy device private keys, invent cross-device campaign synchronization, or treat a Steam account as an Identity account. Paket credentials move only through an explicit supported Identity device-approval/linking flow.

### 4. Unify input before adding screen buttons

Read keyboard/mouse, gamepad and touch into analog `move` and `look` values plus pressed/released actions. Touch camera movement does not depend on browser Pointer Lock. Clear held input on cancellation, lost focus, app suspension and mode changes. Menus consume their own input; closing an overlay cannot fire a weapon or ring a bell accidentally.

Landscape is the initial game layout, with a usable rotate prompt where browser orientation locking is unavailable. Left thumb moves; right-side drag controls camera; a context button interacts. Present equipment/actions in a wheel or compact expandable selector instead of showing every desktop hotkey simultaneously. Maintain independent touch identities for moving, aiming and acting together. Provide sensitivity and left-handed layout settings.

| Game/mode | Touch interaction |
| --- | --- |
| Paket on foot | Move/look, context interaction; parcel return/exchange/set-aside remain discoverable. |
| Paket van/dog/photo/dialog | Mode-specific steering/brake or bite/jump; camera shutter; directly selectable bell/dialog choices. |
| Feed the Poor | Aim and throw from a readable shoulder camera; switch cash/food/tools; activate abilities, contextual ATM/shop interaction and vehicle controls. |
| Feed late progression | Open the same shop/charity/vehicle actions without covering the aiming region or losing movement state. |
| Repo City | Tap a building to approach, drag to orbit, pinch to zoom; read source/diff/blame in a panel; optional free-flight controls stay secondary. |

Controller uses the same actions and explicit UI focus. Steam Deck is not advertised as supported solely because a controller resource exists.

### 5. Preserve Paket mesh and Identity contracts

Native gossip and browser `paket-web/3` are not currently interoperable. Define one versioned application session contract, with adapters for the existing transports where practical. First prove a desktop-native/browser pair; then test both mobile app targets. Do not replace MetaMesh with Steam matchmaking just to package the executable.

The host still owns route, shift clock and first completion per address/day. Late joins and reconnects receive the same receipts; credits remain personal to the delivering player. Resume the same mesh identity/invitation after a recoverable restart. Unsupported protocol versions produce a useful upgrade message rather than a hanging join.

A private native-only test build can precede cross-play. A public full-parity Paket release waits for native/browser invitation, reconnect, delivery and signed-career tests. A phone cannot be assumed to host while its OS suspends the app. Persist the host snapshot and visibly handle a temporarily unavailable host; do not promise uninterrupted background hosting or add an unplanned dedicated server.

### 6. Reuse map services and handle lifecycle failures

Native adapters call the existing cached map/search/elevation services with cancellation, bounded timeouts and fallback behavior. Replace browser-relative `/api/...` calls with explicit configured service origins, including Feed exchange-rate/wealth requests. Prefer the current map proxy/cache over sending each app installation directly to public Overpass. Keep arbitrary coordinate selection and runtime streaming; bundled neighborhoods are optional first-load caches, not a geographic restriction.

Provide one centered loading/error flow with stage/progress where known, retry and location change. Missing GPU support, failed assets and unavailable maps terminate in recoverable states. Cached geometry should allow its supported offline scope; new uncached geography can require a connection.

Pause Feed's solo simulation and save on backgrounding; do not silently reset the campaign on foregrounding. Paket preserves/reconciles session state rather than pausing everybody's shared clock. Recreate a lost graphics surface, release audio/input correctly and retry interrupted requests. Mobile app assets are packaged content, not a downloaded WASM application.

### 7. Measure device support

Run an early build/render/input spike on the pinned Bevy version before promising minimum OS/GPU versions. Narrow desktop-only dependencies such as `arboard`, AppKit or filesystem watcher features to supported targets. Android uses the native Android/Gradle/NDK route; iOS uses an Xcode application with native rendering. Follow the pinned engine's mobile examples rather than treating the Bevy examples on `main` as a guaranteed compatible build recipe.

Mobile-web baseline is a supported WebGPU browser. Safari enables WebGPU on iOS/iPadOS 26; Android support depends on browser, OS and GPU. A working desktop browser is not evidence of a working phone. Unsupported devices get a clear compatibility screen before map loading. A WebGL2 renderer is a separate feasibility/performance task, not a promised one-line fallback.

Record real-device startup time, peak memory, frame-time percentiles, thermal behavior and streaming failures. Initial acceptance target: stable 60 FPS on the declared desktop test machine and 30 FPS minimum on declared mobile devices during a representative 20-minute session, with mobile p95 frame time at most 50 ms. Define the scene and hardware before accepting results. Prefer reduced shadows, distance, density and batching before aggressive resolution reduction. Keep UI/text sharp; expose quality override and recalibration. Simulator and automated desktop touch emulation do not replace phone GPU/thermal checks.

### 8. Scope Repo City by repository source

Retain the native VibePad and existing desktop review/persistence model. Mobile initially opens GitHub repository/PR snapshots using the existing browser source model, with token storage and device authentication adapted to mobile. It does not run a desktop Git subprocess or gain access to a Mac's watched checkout by sharing an account.

Make approach/orbit and panel reading the default touch mode. Preserve repository/revision/file/line identity when changing views. Support search, source/diff/blame, line selection and private draft notes; publish only on an explicit action. Cap nearby rendered buildings and load source on demand. Prefer an iPad/tablet review milestone before claiming phone ergonomics. Unsupported local-repository/MCP actions must be explained before selection.

Steam accepts only specified categories of non-game software. Confirm Repo City's category eligibility before treating Steam publication as available; ordinary signed desktop distribution remains independent of that decision.

## Implementation order and estimates

All ranges are **working days for one developer with agent assistance**, starting from current source, including local acceptance work and a distributable release candidate. They exclude store waiting/review, external account setup delays, new gameplay, localization expansion and unrelated visual redesign. They are alternative paths from the same baseline, **not numbers to add together**. A working week means five working days. Shared controls and UI/storage work are reused across targets and, where demonstrated, between the games.

| Candidate from current source | Feed the Poor | Paket | Repo City / Vibe City |
| --- | ---: | ---: | ---: |
| Mobile web, supported WebGPU phones | 5–8 days | 7–12 days | 10–18 days |
| Native Steam Windows candidate | 15–25 days | 20–35 days | 10–18 days, category eligibility unresolved |
| Native Android candidate | 20–35 days | 25–45 days | 25–45 days, snapshot review |
| Native iPhone/iPad candidate | 25–40 days | 30–50 days | 30–50 days, snapshot review |

After shared native UI/storage and touch controls are complete, the incremental platform work is provisionally 4–8 days for Android and 5–10 for iOS. These are included in the from-current-source ranges, not added on top. Desktop macOS/Linux packages after Windows provisionally add 2–5 days per OS and game, subject to dependency/device results. Older-browser WebGL2 support, if feasible, provisionally adds 8–15 days; it is outside the baseline.

Stages, in dependency order:

1. **Feasibility and contract inventory, 1–2 days within the ranges.** Build the pinned renderer on real Android/iOS hardware; inventory UI commands/save ownership; identify incompatible dependencies. Revise estimates if the renderer cannot run.
2. **Feed native vertical slice.** Select a character/location, load a street, move/aim/feed, withdraw money, save/relaunch and retain the end report. This proves native UI/storage without Paket's extra session dependency.
3. **Complete native game UI and Paket parity.** Port remaining game flows; implement the common session contract and Identity/storage integration. Keep existing web flows working.
4. **Touch and controller.** Introduce shared analog actions, then verify complete mode-specific flows in mobile web and native builds. Mobile web can validate controls before store packages are ready.
5. **Native packaging and device qualification.** Produce Windows packages and Android/iOS candidates, verify clean installs, upgrades and sustained play. Treat Repo City as a separately scheduled product, not an automatic side effect.
6. **Store submission.** Upload tested artifacts to private/test tracks, finish metadata and required disclosures, complete review, then perform the authorized release.

## Store clocks and costs

Rules were checked on 17 November 2026; recheck before submission. They do not reduce engineering work and are not guaranteed completion dates.

| Channel | External requirements relevant to scheduling |
| --- | --- |
| Steam | US$100 per product, recoupable after US$1,000 adjusted gross revenue. Steam Direct describes 15–30 days for onboarding, a 21-day app-fee wait for initial titles and at least two weeks of public Coming Soon. Waiting periods may overlap. Store/build review is normally 3–5 business days; Valve recommends allowing at least seven for corrections. |
| Google Play | US$25 one-time developer account registration. New personal accounts created after 13 November 2023 require 12 opted-in closed testers continuously for 14 days before applying for production access. This condition is account-dependent; review/access approval is additional. |
| Apple App Store | Apple Developer Program costs US$99 per membership year, with local pricing variations. Use signed device builds and TestFlight before App Review; the first external TestFlight build requires beta review. Do not promise a fixed App Review completion time. |
| Mobile web | No app-store submission clock. Browser compatibility, deployment and device verification still apply. |

Steam store descriptions must match tested launch features. Disclose player-facing generated art in the Steam content survey where required; generated portraits/posters/newspapers are distinct from ordinary procedural OSM geometry. Preserve asset licenses and attribution. Existing real names/caricatures remain the content baseline; store acceptance is an external decision, not a consequence of choosing a native renderer.

## Verification and release gates

- Author Given/When/Then acceptance tests before behavior changes. Prove failures first for logic/integration work; execute browser UI verification after implementation. Native UI needs native app interaction and screenshots, not Playwright against an unrelated website.
- Keep focused Rust rule/action/save/session tests. Use isolated Playwright for the real mobile-web routes, including unsupported GPU, pending/failed map, touch cancellation and retained report states.
- Exercise native windows and packaged builds with keyboard/controller; use native device tooling and actual touch for Android/iOS. Include overlay focus, Unicode text, safe areas, orientation, background/foreground, network loss and install upgrades.
- Verify Paket with native/native and native/browser pairs, a late join, host restart, duplicate receipt and a suspended phone. Verify Feed money/fortune invariants and a large charity save across restart without compressing the game's long progression.
- Test packages without a checkout/toolchain, from another working directory and with a read-only installation. Record minimum tested device/browser versions; label untested platforms explicitly.
- Keep existing web CI green. Add native package jobs only when their local build is reproducible. Release a batched verified candidate rather than using repeated CI pushes as the development loop.

## Official references

- [Bevy platform-specific mobile examples](https://github.com/bevyengine/bevy/blob/main/examples/README.md#platform-specific-examples) establish the engine's Android/iOS routes; use the version matching the project when implementing.
- [WebGPU implementation status](https://github.com/gpuweb/gpuweb/wiki/Implementation-Status) and [Safari 26 WebGPU support](https://webkit.org/blog/17333/webkit-features-in-safari-26-0/) describe browser support, not game certification.
- [Steam Direct](https://partner.steamgames.com/steamdirect), [onboarding](https://partner.steamgames.com/doc/gettingstarted/onboarding), [review](https://partner.steamgames.com/doc/store/review_process) and [content survey](https://partner.steamgames.com/doc/gettingstarted/contentsurvey) cover publication requirements.
- [Google Play registration](https://support.google.com/googleplay/android-developer/answer/6112435?hl=en) and [new personal account testing](https://support.google.com/googleplay/android-developer/answer/14151465?hl=en) cover fees and production access.
- [Apple enrollment](https://developer.apple.com/help/account/membership/program-enrollment/), [TestFlight](https://developer.apple.com/testflight/) and [App Review Guidelines](https://developer.apple.com/app-store/review/guidelines/) cover distribution and review.
