import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "src") },
  },
  server: {
    port: 5300,
    proxy: {
      // dev: 把后端路径代理到 sub2org server，避免 CORS
      "/v1": "http://localhost:3300",
      "/auth": "http://localhost:3300",
      "/admin": "http://localhost:3300",
      "/healthz": "http://localhost:3300",
    },
  },
});
