import { defineConfig } from "@playwright/test";

// End-to-end tests run against the production build (`npm run build`) served
// by `vite preview`, and against the document server when it is running on
// port 8080 (the collaboration test skips otherwise). Set PW_CHROMIUM to use
// a pre-installed browser instead of Playwright's download.
export default defineConfig({
  testDir: "./e2e",
  timeout: 60_000,
  retries: process.env.CI ? 1 : 0,
  use: {
    baseURL: "http://localhost:4173",
    viewport: { width: 1400, height: 900 },
    launchOptions: {
      executablePath: process.env.PW_CHROMIUM,
      args: ["--use-gl=swiftshader", "--enable-unsafe-swiftshader"],
    },
  },
  webServer: {
    command: "npx vite preview --port 4173 --strictPort",
    url: "http://localhost:4173",
    reuseExistingServer: true,
  },
  reporter: process.env.CI ? "github" : "list",
});
