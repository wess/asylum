import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

const page = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  base: "./",
  build: {
    // The only chunk over the default 500 kB warning is the Three.js hero scene,
    // which is now a lazily `import()`-ed chunk (see src/home.ts) — it is not in
    // any page's initial payload and is skipped entirely under reduced-motion /
    // data-saver. This explicit budget acknowledges that single deferred chunk.
    chunkSizeWarningLimit: 600,
    rollupOptions: {
      input: {
        home: page("./index.html"),
        docs: page("./docs/index.html"),
        book: page("./book/index.html"),
        videos: page("./videos/index.html"),
        tutorials: page("./tutorials/index.html"),
      },
    },
  },
});
