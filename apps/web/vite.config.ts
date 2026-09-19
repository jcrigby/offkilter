import { defineConfig } from "vite";

export default defineConfig({
  build: { target: "es2022" },
  server: {
    port: 5173,
    // The document server (cargo run -p ok-server) during development.
    proxy: { "/api": { target: "http://localhost:8080", ws: true } },
  },
});
