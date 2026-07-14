import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";

const page = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  base: "./",
  build: {
    rollupOptions: {
      input: {
        home: page("./index.html"),
        docs: page("./docs/index.html"),
        tutorials: page("./tutorials/index.html"),
      },
    },
  },
});
