import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {defineConfig} from '@playwright/test';
export default defineConfig({
  testDir:'./tests', workers:1, timeout:120_000,
  outputDir:process.env.PAKET_TEST_OUTPUT || join(tmpdir(),'paket-playwright'),
  use:{channel:process.env.PAKET_CHROME_CHANNEL,baseURL:`http://127.0.0.1:${process.env.PAKET_TEST_PORT||8797}`,viewport:{width:1440,height:900},launchOptions:{args:['--enable-unsafe-webgpu']},trace:'retain-on-failure'},
  webServer:{command:`wrangler dev --port ${process.env.PAKET_TEST_PORT||8797} --inspector-port ${process.env.PAKET_TEST_INSPECTOR||9237}`,url:`http://127.0.0.1:${process.env.PAKET_TEST_PORT||8797}`,reuseExistingServer:false,timeout:60_000},
});
