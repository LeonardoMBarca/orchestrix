import {defineConfig} from '@playwright/test';

const prototypePort=process.env.ORCHESTRIX_PROTOTYPE_PORT||4173;
const prototypeURL=`http://127.0.0.1:${prototypePort}`;

export default defineConfig({
  testDir:'./tests',
  fullyParallel:true,
  workers:2,
  timeout:20000,
  use:{channel:'chrome',baseURL:prototypeURL,viewport:{width:1440,height:960}},
  webServer:{command:'node server.mjs',url:prototypeURL,reuseExistingServer:!process.env.CI},
  reporter:'list',
});
