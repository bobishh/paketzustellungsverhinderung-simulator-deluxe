import {homedir} from 'node:os';
import {join} from 'node:path';
import {defineConfig} from '@playwright/test';
const firefox=process.env.RICH_BROWSER==='firefox';
export default defineConfig({
  testDir:'./tests', workers:1, timeout:120_000,
  outputDir:process.env.PAKET_TEST_OUTPUT || join(homedir(),'Workspace/Codex/paket/.work/playwright'),
  use:{headless:true,browserName:firefox?'firefox':'chromium',channel:firefox?undefined:process.env.PAKET_CHROME_CHANNEL||'chromium',baseURL:`http://127.0.0.1:${process.env.PAKET_TEST_PORT||8797}`,viewport:{width:1440,height:900},launchOptions:firefox?{firefoxUserPrefs:{'dom.webgpu.enabled':true,'gfx.webrender.all':true}}:{args:['--enable-unsafe-webgpu']},trace:{mode:'retain-on-failure',screenshots:false,snapshots:true,sources:true},screenshot:'only-on-failure'},
  webServer:{command:`./node_modules/.bin/wrangler dev --port ${process.env.PAKET_TEST_PORT||8797} --inspector-port ${process.env.PAKET_TEST_INSPECTOR||9237}`,url:`http://127.0.0.1:${process.env.PAKET_TEST_PORT||8797}`,reuseExistingServer:false,timeout:60_000},
});
