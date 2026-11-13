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

The wasm-bindgen CLI version must match the version in Cargo.lock. The local MetaMesh packages are expected in `../mesh/packages/`; build their `dist/` files and the `mesh-transport/transport-wasm/` runtime first if missing.

For development, run Wrangler from `web/` on port 8788 and `npm run dev` in another terminal (port 4188). For a production preview, serve `web/dist` through the Worker so large precompressed WASM modules and map requests are handled correctly.

The deployment configuration targets:

`paketzustellungsverhinderung-simulator-deluxe.meta-uber-engineer.dev`

Run `wrangler deploy` from `web/` with the existing Cloudflare account. This creates the separate Worker and its custom domain; it does not replace the blog Worker.

## Identity and multiplayer

Play as a guest or choose **Sign in with Identity**. The existing Identity portal signs authorization for this browser's device. The app verifies the challenge, origin, certificate chain and signed approval before adopting the identity. Private keys stay in browser storage. Returning from sign-in preserves a ride invitation.

Browser multiplayer uses the existing MetaMesh Iroh WASM transport and relay. Start a ride, copy its invite from the map, and open it in another browser. The host must remain open. Browser invitations currently use `paket-web/1`; desktop gossip sessions are not interoperable with this browser protocol yet. The world is still selected by coordinates, not by a content-addressed world bundle.

## Controls

- WASD: walk or drive; V: enter/leave the van.
- E: pick up, place or hand over a parcel. A correct handover earns 0 KPI.
- F: camera; Space / left mouse: take a delivery photo. In the browser, **Download photo** saves the latest PNG.
- M: map and ride invitations; Tab: release cursor; hold F1: full controls.

## Checks

```sh
cargo test -p paket-game -p paket-geo
cd web
npm run check
npm test
```

Map geometry comes from OpenStreetMap, with bundled public neighbourhood data for first load and a bounded, cached Overpass endpoint for additional areas. See THIRD_PARTY.md for licenses.
