## 1. Establish the native and mobile baseline

- [ ] 1.1 Inventory current UI actions, state projections and save schemas in both game repositories; identify their Rust owners and required parity flows.
- [ ] 1.2 Build a minimal scene and UI with the pinned Bevy version on a real Android device and iPhone/iPad; check touch, audio, surface recreation and incompatible dependencies.
- [ ] 1.3 Record the supported device/browser matrix, representative performance scenes and revised effort ranges after the spike.
- [ ] 1.4 Write outer acceptance scenarios for native launch/play/save/report and mobile-web play, including one relevant loading/error state per flow.

## 2. Deliver native UI and persistence

- [ ] 2.1 Add typed game actions/state projections, keeping browser serialization at its boundary and existing game rules authoritative.
- [ ] 2.2 Complete Feed the Poor's native vertical slice: character/location, map loading, HUD, aiming/feeding, ATM, pause, checkpoint restore and retained newspaper report.
- [ ] 2.3 Complete Feed's native shops, abilities, vehicles, bosses, progression and charity management without changing economy scale.
- [ ] 2.4 Complete Paket's native lobby, arbitrary location search/picker, HUD/minimap/roster, intercom/dialogs, parcels/photos, settings/help and profile UI.
- [ ] 2.5 Add transactional native SQLite storage, migrations, valid import/export and platform credential handling; test large center collections and signed career records.
- [ ] 2.6 Package assets and resolve writable data/cache/photo paths through OS directories; remove checkout-path assumptions from packaged execution.
- [ ] 2.7 Add explicit native API origins, cached map/search/elevation adapters and recoverable loading/GPU/network failures. Keep browser UI working through migration.

## 3. Preserve Paket sessions and identity

- [ ] 3.1 Specify a shared versioned application session contract for native/browser transports; retain host authority and signed credit ownership.
- [ ] 3.2 Prove invitations, public discovery, join/share, late join and duplicate delivery behavior between native and browser clients.
- [ ] 3.3 Add native Identity device approval and career persistence without substituting Steam IDs or copying browser device keys implicitly.
- [ ] 3.4 Verify restart/reconnect uses the saved mesh identity, route, clock and receipts; show suspended/unavailable host and incompatible-version states.

## 4. Add portable controls

- [ ] 4.1 Introduce analog move/look and contextual pressed/released actions; adapt existing keyboard/mouse without changing their behavior.
- [ ] 4.2 Add controller bindings, focus navigation and complete action selection; test disconnect/reconnect and mode changes.
- [ ] 4.3 Add multitouch movement, camera drag, contextual interaction, equipment wheel, sensitivity and left-handed layout without requiring Pointer Lock.
- [ ] 4.4 Verify Paket parcel return/exchange, intercom, van, dog and photo flows using only touch.
- [ ] 4.5 Verify Feed cash/food aim-and-throw, ATM/PIN/call, shop, abilities, late-progression tools and vehicles using only touch.
- [ ] 4.6 Clear held actions on cancellation/focus loss/backgrounding; ensure UI taps do not leak into gameplay and UI stays sharp at reduced scene resolution.

## 5. Qualify mobile web and native packages

- [ ] 5.1 Run isolated Playwright acceptance on each real mobile-web route, including pending/error/unsupported GPU states and canceled multitouch.
- [ ] 5.2 Verify mobile web on declared Android GPUs and Safari/iOS devices; explain unsupported browsers before loading maps.
- [ ] 5.3 Create reproducible native Windows release packages; test clean/read-only installs, other working directories, Unicode paths and save-preserving upgrades.
- [ ] 5.4 Create native Android arm64 Gradle/NDK packages and signed test AABs; verify Back, safe areas, permissions, app lifecycle and persistent data.
- [ ] 5.5 Create native iPhone/iPad Xcode builds and TestFlight candidates; verify input, text entry, permissions, background/foreground and save-preserving upgrades.
- [ ] 5.6 Meet documented startup/memory/frame-time/20-minute thermal targets on actual devices; retain quality override and sharp HUD rendering.
- [ ] 5.7 Add separately verified macOS/Linux packages when scheduled; test Steam Deck before advertising controller/Deck support.
- [ ] 5.8 Keep WebGL2 outside baseline until a separate feasibility check establishes renderer compatibility and acceptable performance.

## 6. Adapt Repo City separately

- [ ] 6.1 Confirm the product's Steam software-category eligibility before scheduling a Steam publication.
- [ ] 6.2 Retain native VibePad/storage and define mobile snapshot source/auth contracts without desktop worktree/watcher/MCP assumptions.
- [ ] 6.3 Implement touch approach/orbit/zoom, search, readable source/diff/blame, line selection and private notes; preserve explicit publishing actions.
- [ ] 6.4 Validate a tablet review flow first, then phone ergonomics, large-repository streaming, lifecycle and auth failure states.
- [ ] 6.5 Produce reproducible signed desktop/mobile candidates with supported capabilities stated accurately.

## 7. Prepare distribution after candidate acceptance

- [ ] 7.1 Configure Steam apps/depots/private betas, actual gameplay screenshots, system requirements, ratings, attribution and required AI-content disclosures.
- [ ] 7.2 Complete Google Play account-dependent closed testing and Apple TestFlight/device review; prepare accurate privacy and capability declarations.
- [ ] 7.3 Run required local checks, then final CI/package verification on the release revisions; preserve the tested artifacts for submission.
- [ ] 7.4 Complete store review and applicable waiting periods before the separately authorized public release; do not advertise unverified platform or cross-play support.
