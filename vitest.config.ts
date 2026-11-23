import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

// Separate from vite.config.ts on purpose: that file carries Tauri dev-server
// settings (fixed port, HMR host) that must not apply to tests.
export default defineConfig({
  plugins: [vue()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.spec.ts"],
  },
});
