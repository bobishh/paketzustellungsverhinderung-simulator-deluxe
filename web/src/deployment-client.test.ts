import { expect, it } from 'vitest';
import { deploymentClient } from '../deployment-client';
it('Given no deployment dependency, when building the game, then attach no browser client', () => {
  expect(deploymentClient({})).toEqual([]);
  expect(deploymentClient({ ANALYTICS_CLIENT_URL: 'https://client.example.test/extension.js' })).toEqual([]);
});
it('Given explicit deployment settings, when building, then load only the supplied external dependency', () => {
  expect(deploymentClient({ ANALYTICS_CLIENT_URL: 'https://client.example.test/extension.js', ANALYTICS_PROJECT: 'fixture', ANALYTICS_BROWSER_TOKEN: 'public-fixture-key' })).toEqual([
    { tag: 'script', attrs: { src: 'https://client.example.test/extension.js', defer: true, 'data-project': 'fixture', 'data-token': 'public-fixture-key' }, injectTo: 'head' },
  ]);
});
it('Given an unsafe dependency URL, when building, then reject it', () => {
  expect(() => deploymentClient({ ANALYTICS_CLIENT_URL: 'javascript:alert(1)', ANALYTICS_PROJECT: 'fixture', ANALYTICS_BROWSER_TOKEN: 'public-fixture-key' })).toThrow();
});
