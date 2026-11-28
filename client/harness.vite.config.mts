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
      // ORDER MATTERS: the alias plugin matches a prefix, so the specific entry
      // must precede `@` or `@/hooks/use-subscription` resolves through it to the
      // real module (and the harness dies on a Tauri call that has no window).
      //
      // The subscription chip's data source, replaced ONLY for the harness: the
      // real hook needs the Tauri IPC, and the chip's state→label mapping is what
      // the `subscription` scenario renders. The component itself is the real one.
      '@/hooks/use-subscription': path.resolve(
        '.harness/subscription-fixture.ts',
      ),
      '@': path.resolve('./src'),
      '@root': path.resolve('.'),
    },
  },
  define: { OS_PLATFORM: `"linux"` },
})
