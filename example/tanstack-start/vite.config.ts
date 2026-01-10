import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import viteReact from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import viteTsConfigPaths from "vite-tsconfig-paths";

const config = defineConfig({
  plugins: [viteTsConfigPaths(), tanstackStart(), viteReact()],
  ssr: {
    noExternal: ["@takumi-rs/core", "@takumi-rs/image-response"],
  },
  optimizeDeps: {
    exclude: ["@takumi-rs/core"],
  },
});

export default config;
