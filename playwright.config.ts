import { defineConfig, devices } from "@playwright/test";

const e2ePort = process.env.XAC_E2E_PORT ?? "5174";
const e2eBaseURL = `http://127.0.0.1:${e2ePort}`;

export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 60_000,
  expect: {
    timeout: 5_000
  },
  fullyParallel: true,
  reporter: [["list"]],
  use: {
    baseURL: e2eBaseURL,
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
    url: e2eBaseURL,
    reuseExistingServer: false,
    timeout: 60_000,
    env: {
      VITE_XAC_MOCK_IPC: "1",
      XAC_E2E_PORT: e2ePort
    }
  }
});
