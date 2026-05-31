import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 30_000,
  expect: {
    timeout: 5_000
  },
  fullyParallel: true,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:5174",
    headless: true,
    trace: "on-first-retry",
    viewport: { width: 1440, height: 900 }
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] }
    }
  ],
  webServer: {
    command: "npm run dev:e2e",
    url: "http://127.0.0.1:5174",
    reuseExistingServer: false,
    timeout: 30_000,
    env: {
      VITE_XAC_MOCK_IPC: "1"
    }
  }
});
