/** A deployment may attach an external browser extension; source builds omit it. */
export function deploymentClient(environment: NodeJS.ProcessEnv) {
  const src = environment.ANALYTICS_CLIENT_URL?.trim();
  const project = environment.ANALYTICS_PROJECT?.trim();
  const token = environment.ANALYTICS_BROWSER_TOKEN?.trim();
  if (!src || !project || !token) return [];
  const url = new URL(src);
  if (url.protocol !== 'https:' || url.username || url.password || url.hash) throw new Error('Use an HTTPS browser client without URL credentials');
  return [{ tag: 'script', attrs: { src: url.href, defer: true, 'data-project': project, 'data-token': token }, injectTo: 'head' as const }];
}
