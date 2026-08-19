import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { fileURLToPath, URL } from 'node:url';

// Vite config for the DiskScope web layer (Tauri webview frontend).
// - base './' so the built assets resolve relative to the Tauri asset
//   protocol (tauri://localhost / http://tauri.localhost).
// - `port` matches what tests/visual_gate.sh expects (GATE3_PORT=5173).
// - `clearScreen: false` keeps Tauri dev output visible.
export default defineConfig({
  plugins: [react()],
  base: './',
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
  },
});
