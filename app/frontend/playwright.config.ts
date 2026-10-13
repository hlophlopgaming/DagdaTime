import {defineConfig} from '@playwright/test';
export default defineConfig({
  testDir:'./e2e',timeout:30_000,workers:1,
  use:{baseURL:'http://127.0.0.1:5173',viewport:{width:1280,height:850},launchOptions:{executablePath:process.env.CHROMIUM_PATH}},
  webServer:{command:'npm run dev',url:'http://127.0.0.1:5173',reuseExistingServer:!process.env.CI},
});
