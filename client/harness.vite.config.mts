import path from 'node:path'

import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import svgr from 'vite-plugin-svgr'

/** The rendering harness. `root` is `.harness/`, aliases mirror the app's. */
export default defineConfig({
  root: path.resolve('.harness'),
  base: './',
  server: { port: 3100, strictPort: true },
  plugins: [svgr(), react()],
  resolve: {
    alias: {
      '@': path.resolve('./src'),
      '@root': path.resolve('.'),
    },
  },
  define: { OS_PLATFORM: `"linux"` },
})
