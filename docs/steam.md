# Steam release plan

Status: 13 November 2026. The game already has a native Rust/Bevy client. A Steam release can package that executable; it does not require replacing MetaMesh or wrapping the public website. There is no Steam AppID, depot configuration or upload automation in this repository yet.

## What is ready and what is missing

| Area | Current state | Before the first public build |
| --- | --- | --- |
| Game | Native and WASM share the Bevy world, courier rules, shift clock and dogs. | Build and smoke-test a Windows x64 release on a clean machine. Add Linux/macOS only when their packages are tested. |
| Files | Assets, cache and screenshots in `main.rs` use compile-time source paths. | Resolve shipped assets relative to the installed executable; use writable OS user-data directories for saves, cache and photos. Test launching from a different working directory and a read-only install directory. |
| Maps | Geography streams from OSM services; the six browser starter neighbourhoods are prepared separately. | Package versioned starter geometry for a reliable first shift; provide loading, retry and failure states for online areas. Keep OSM attribution and third-party notices in the package. |
| Interface | Address search, minimap, Identity sign-in and career UI live in `web/src`. The native client has the older map UI. | Bring the promised player experience to the native client. A native executable alone does not include the DOM interface. |
| Multiplayer | Native peers use gossip; browsers use `paket-web/2`. | Decide whether the first release supports native peers only or complete cross-client protocol compatibility. Test invitations, reconnects and host departure on separate machines. |
| Career | Signed browser career records persist in IndexedDB and replicate through joined games. | Add native durable storage and Identity approval/callback handling; keep the same signed-record merge rules. A Steam ID does not automatically become an Identity key. |
| Distribution | CI tests the native code and produces a browser artifact. | Add a separate Windows release package, executable metadata/icon, required runtime dependencies and a SteamPipe upload job targeting a private beta. CI's browser artifact is not a Steam depot. |
| Player controls | Keyboard and mouse. | Provide a clear pause/quit/settings flow. Controller support and Steam Deck testing are a separate milestone; do not advertise them before testing. |

Suggested first milestone: a Windows-only private Steam beta with one bundled neighbourhood, solo play and native-to-native multiplayer. Expand the scope after the clean-install and two-machine checks pass.

## Steam setup

1. Complete Steamworks onboarding: publisher identity, bank and tax details. Valve allows onboarding as an individual. Pay the **US$100 (or local equivalent) Steam Direct fee** for the app; Valve recoups it after the product reaches US$1,000 adjusted gross revenue.
2. Create the app and configure a Windows depot, launch executable, supported languages and operating systems. Put only redistributable game content in the depot.
3. Prepare the store page: description, actual gameplay screenshots, capsule/library art, pricing and system requirements. Complete the content survey and age-rating information; a valid rating is required for availability in Germany.
4. Use the Steamworks SDK's SteamPipe tools to upload a build and activate it on a private beta branch. Store upload credentials outside the public repository. Keep publishing the default branch a separate explicit release action.
5. Submit the store page and build for Valve review. Current onboarding documentation specifies a **21-day wait after paying the app fee** for the first titles and at least **two weeks of public Coming Soon** before release. Use the dates shown in the actual Steamworks dashboard when scheduling.

Runtime Steamworks API integration is optional. The SDK's upload tooling is required for the SteamPipe workflow; Steam achievements, friends/lobby invites, overlay integration and Cloud saves can follow later. MetaMesh can remain the game network. If Steam authentication is added, it needs an explicit account-linking design with Identity rather than silently replacing the player's existing identity.

## Acceptance checks for a package

- Install on a machine without this source checkout or developer toolchains; start through Steam and directly from another working directory.
- Complete a shift with the bundled map when the map upstream is unavailable. Verify online-area failures have a recoverable UI.
- Save, quit, update and restart without losing the profile; verify installation directories need no writes.
- Connect two native clients, exchange deliveries and late-join the same shift clock. Only advertise browser cross-play after it passes the same test.
- Confirm screenshots, input settings, quit flow, display scaling and minimum supported GPU behavior on the target OS.

## Official references

- [Onboarding and release waiting periods](https://partner.steamgames.com/doc/gettingstarted/onboarding)
- [Steam Direct fee](https://partner.steamgames.com/doc/gettingstarted/appfee)
- [Steamworks SDK and optional runtime features](https://partner.steamgames.com/doc/sdk)
- [SteamPipe, depots and launch options](https://partner.steamgames.com/doc/sdk/uploading)
- [Store and build review](https://partner.steamgames.com/doc/store/review_process)
- [Content survey](https://partner.steamgames.com/doc/gettingstarted/contentsurvey)
- [Age ratings in Germany](https://partner.steamgames.com/doc/gettingstarted/contentsurvey/germany)
