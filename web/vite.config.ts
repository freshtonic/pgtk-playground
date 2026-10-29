import { defineConfig } from "vite";

export default defineConfig({
  // GitHub Pages serves the site at https://freshtonic.github.io/pgtk-playground/.
  base: "/pgtk-playground/",
  // PGlite finds its wasm and data files next to its own module.
  optimizeDeps: { exclude: ["@electric-sql/pglite"] },
  worker: { format: "es" },
  build: { target: "es2022" },
});
