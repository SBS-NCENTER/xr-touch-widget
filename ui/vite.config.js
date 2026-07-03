import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// package.json is "type": "module", so __dirname must be derived
const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [svelte()],
  build: {
    rollupOptions: {
      input: {
        widget: resolve(__dirname, 'widget.html'),
        settings: resolve(__dirname, 'settings.html'),
        demo: resolve(__dirname, 'demo.html'),
        harness: resolve(__dirname, 'harness.html'),
      },
    },
  },
  // Tauri dev: fixed port so tauri.conf.json devUrl can point here
  server: { port: 5173, strictPort: true, host: true },
});
