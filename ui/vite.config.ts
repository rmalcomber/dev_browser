import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';

// Relative base: the shell is served from app://shell/, never from a web server.
export default defineConfig({
  base: './',
  plugins: [preact()],
  build: { outDir: 'dist', target: 'es2022', rollupOptions: { input: { index: 'index.html', launcher: 'launcher.html', panel: 'panel.html' } } },
  test: { environment: 'jsdom' },
});
