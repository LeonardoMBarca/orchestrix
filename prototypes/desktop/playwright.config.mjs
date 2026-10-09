import {defineConfig} from '@playwright/test';

export default defineConfig({
  testDir:'./tests',
  fullyParallel:true,
  workers:2,
  timeout:20000,
  use:{channel:'chrome',baseURL:'http://127.0.0.1:4173',viewport:{width:1440,height:960}},
  webServer:{command:'node server.mjs',url:'http://127.0.0.1:4173',reuseExistingServer:!process.env.CI},
  reporter:'list',
});
