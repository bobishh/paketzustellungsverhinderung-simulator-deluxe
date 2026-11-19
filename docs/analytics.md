# Optional browser extensions

The game emits local `paket-ui-action`, `paket-mode`, `paket-progress` and related
events. The source application contains no analytics transport or collector
payload implementation.

A hosting deployment can set `ANALYTICS_CLIENT_URL`, `ANALYTICS_PROJECT` and
`ANALYTICS_BROWSER_TOKEN` before building. All three must be present. The build
then adds an external HTTPS script with its public browser credential. Without
these settings, no extension is loaded. Keep addresses and keys in deployment
configuration outside the source repository.

The external producer owns event selection, privacy filtering, serialization
and network requests. Its failure must not interrupt the game.
