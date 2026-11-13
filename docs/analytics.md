# Game analytics

TinSink project: `paketzustellungsverhinderung-simulator-deluxe`.
Console: <https://analytics.meta-uber-engineer.dev/admin/logs>.
Its owner is the Identity configured as `SINK_TRUSTED_OWNER` in the existing service deployment.

Browser writes are restricted to the exact production origin:
`https://paketzustellungsverhinderung-simulator-deluxe.meta-uber-engineer.dev`.
The public browser write key is in `web/src/analytics-config.ts`; it is separate from server and administrator credentials.
Project registration is stored by TinSink, so adding this project required no service deployment.

`initGameAnalytics()` runs after the Identity callback fragment has been consumed.
Local development, previews, `?press-shot`, Do Not Track and Global Privacy Control do not collect events.
The client sends no cookies, request referrer, persistent visitor ID, mesh ID, Identity keys, names, address searches, coordinates or invitation URLs.
Page paths are fixed to `/`, excluding query strings, hash fragments and callback paths.
TinSink may add its existing server-side request context; the client does not add fingerprinting.

| Stream | Events |
| --- | --- |
| Analytics v1 | Page view; one event after 30 visible, focused, active seconds |
| Telemetry v2 | Play clicked; solo/host/join requested; Identity/profile opened; invite copy requested; address search requested |
| Telemetry v2 | Shift started, left or completed; delivery completed or photographed; intercom rang; van marked; rare delivery bonus |

Only the selected role, aggregate counter increments and elapsed wall time for shift completion/exit accompany gameplay events.
The first progress snapshot establishes a baseline, including for late joins.
Subsequent 10 Hz snapshots produce events only when counters increase; repeated frames do not produce telemetry.
Completed or left shifts reset the collector on the next ride.
HUD route lists and radar payloads are never observed.

Telemetry is batched every five seconds, at 20 events, when the page becomes hidden or on page exit.
The client allows at most 120 telemetry events per minute; the TinSink project has its existing 5,000 events/hour quota.
Network errors are dropped without retries or interruption of gameplay.
This is aggregate product instrumentation, not a reliable transaction ledger.

The shared TinSink `/events` preflight covers multiple projects and may permit an Origin used by an unrestricted project.
The actual POST validates this project's exact Origin and browser key; a foreign Origin receives HTTP 403.
No request uses credentialed wildcard CORS.

Validation: `cd web && npm run check && npm test -- --run src/analytics.test.ts`.
Production checks covered allowed-origin preflight, foreign-origin POST rejection, successful v1/v2 writes and report readback from ClickHouse.
Two identifiable synthetic checks use `game.integration.checked` and `/__analytics_check`; they are separate from gameplay events.
