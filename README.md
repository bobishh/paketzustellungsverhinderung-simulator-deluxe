# Paketzustellungsverhinderung Simulator Deluxe

A neighbourhood courier game. DREHLSS is the fictional delivery company.

## Browser build

The browser runs the same Bevy game and rules as the desktop build, compiled to WebAssembly. It requires WebGPU and keyboard/mouse controls.

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --locked
cd web && npm ci --install-links && cd ..
./scripts/build-web.sh
```

The wasm-bindgen CLI version must match the version in Cargo.lock. The local MetaMesh repository is expected at `../mesh`; its packages export TypeScript directly and its checked-in `mesh-transport/transport-wasm/` runtime must be present. CI pins MetaMesh to the revision in `.github/workflows/ci.yml` so a clean checkout can reproduce the same dependencies.

For development, run Wrangler from `web/` on port 8788 and `npm run dev` in another terminal (port 4188). For a production preview, serve `web/dist` through the Worker so large precompressed WASM modules and map requests are handled correctly.

The deployment configuration targets:

`paketzustellungsverhinderung-simulator-deluxe.meta-uber-engineer.dev`

Run `wrangler deploy` from `web/` with the existing Cloudflare account. This creates the separate Worker and its custom domain; it does not replace the blog Worker.

## Identity and multiplayer

Play as a guest or choose **Sign in with Identity**. The existing Identity portal signs authorization for this browser's device. The app verifies the challenge, origin, certificate chain and signed approval before adopting the identity. Private keys stay in browser storage. Returning from sign-in preserves a ride invitation.

Browser multiplayer uses the existing MetaMesh Iroh WASM transport and relay. Choose **Play with friends**, then **Invite a friend** and open the link in another browser. The host must remain open. Browser invitations currently use `paket-web/1`; desktop gossip sessions are not interoperable with this browser protocol yet. The world is still selected by coordinates, not by a content-addressed world bundle.

## Streets, shifts and careers

The starting screen offers Berlin, New York, São Paulo, Tokyo, Cape Town and Sydney, or a street-address search. Address searches run only on form submission, use a cached Photon proxy, and can fall back to Photon directly. `PHOTON_ORIGIN` can replace the proxy upstream. `python3 scripts/cache-cities.py` prepares public OSM starter geometry before a release.

A shift fills a route of up to 12 unique delivery stops as neighbouring map tiles load. Delivery navigation is separate from the parcel in hand: inspecting another address never swaps or teleports a box. Successful handovers and saved delivery photos automatically select the next unfinished parcel. Courtyard passages tagged in OpenStreetMap cut matching openings in building facades and collision geometry.

The browser HUD shows the shift clock, KPI, current delivery, an expandable route and context controls. F1 opens settings with mouse sensitivity, difficulty and the full controls list.

The host owns the shared 09:00–17:00 clock (48 real minutes). Guests receive the current clock when joining. F2 selects Trainee, Courier or Express before the first doorbell. Residents have deterministic names, away slots and different response delays. Ring the marked recipient with R before leaving a parcel. The intercom offers an all-bells timed bonus; encounters open a timed name dialogue. A rare broken-intercom opportunity lets the courier collect the remaining batch from their van and dump it at the marked destination once per shift.

NPC dogs warn for 2.8 seconds before jumping at a nearby visible courier. Leaving their range or sight avoids the attack; a jump is followed by an 8-second cooldown. Player dogs jump with Space and leap when biting with E. G marks another player's van.

The browser minimap uses the loaded street geometry and actual actor positions. Player careers live in the application's IndexedDB store and replicate over the game mesh as signed, application-scoped per-device records. Peers verify Identity certificate chains and signatures before merging monotonic counters. Profiles combine the same person's devices without double-counting replays; new face seeds and guest-to-Identity career attachment persist across reloads. The portrait is procedural. This is peer replication through joined games: independent cross-device discovery through the Identity resource catalogue is not wired yet, and there is no always-online keeper. Clients retain received replicas locally.

The landing image is an in-engine screenshot, reproducible with `?lat=52.4895&lon=13.3908&press-shot`. The scene uses the same actor and dog meshes as play.

## Controls

- WASD: walk or drive; V: enter/leave the van.
- R: recipient intercom; click bells. 1–3: timed delivery dialogue.
- Tab / Shift+Tab: next / previous delivery while the game is focused. In forms and dialogs Tab keeps normal focus navigation.
- Q near your van: return the held parcel, then collect the selected one with E.
- E: pick up, place or hand over a parcel. A correct handover earns 0 KPI.
- Dog: E bite/scratch, Space jump, G mark a van, B bark.
- F: camera; Space / left mouse: take a delivery photo. In the browser, **Download photo** saves the latest PNG.
- M: map and ride invitations; Esc: release cursor; click the game to resume mouse look.
- F1: browser settings; hold F1 for controls in the native build.

## Checks

```sh
cargo fmt -- --check
cargo test --workspace --locked
cd web
npm run check
npm test
```

GitHub Actions runs these checks on pushes to `main`, pull requests and manual dispatches. The browser job also builds the real WASM game and saves a `browser-build` artifact for seven days. The native job includes the two-peer session integration tests on Linux; local sandbox restrictions can prevent their networking setup on macOS. The cached-map terrain test remains ignored unless its external fixture is prepared. CI does not deploy automatically or require Cloudflare credentials.

The CI artifact contains the Worker and browser assets, but does not fetch live city geometry during tests. Run `python3 scripts/cache-cities.py` before a public release to include the six starter neighbourhoods; uncached places use the runtime map endpoint.

Aggregate visits and game events use TinSink with a dedicated project and an exact production-origin allowlist. See [analytics configuration](docs/analytics.md) for events, privacy boundaries and CORS checks.

For the native distribution gaps and Steam publication steps, see [the Steam release plan](docs/steam.md).

Map geometry comes from OpenStreetMap, with bundled public neighbourhood data for first load and a bounded, cached Overpass endpoint for additional areas. See THIRD_PARTY.md for licenses.
