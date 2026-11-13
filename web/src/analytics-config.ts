// Public browser credential: TinSink restricts this key to this exact Origin.
// Server/owner credentials must never be added here.
export const analyticsConfig = {
  project: 'paketzustellungsverhinderung-simulator-deluxe',
  origin: 'https://paketzustellungsverhinderung-simulator-deluxe.meta-uber-engineer.dev',
  endpoint: 'https://analytics.meta-uber-engineer.dev/events',
  browserToken: "b473f50978c79a8e2d473a2634162d9a8ef0347a4f2250bd7acb1a94abed5b26",
} as const;
