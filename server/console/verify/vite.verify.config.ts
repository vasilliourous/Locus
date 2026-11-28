import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// The dev server for the browser harness: the real console, with /api proxied at
// the local stand-in hub (mock-hub.mjs). Not used by any normal build or deploy.
export default defineConfig({
  plugins: [vue()],
  base: '/admin/',
  server: {
    port: 5199,
    strictPort: true,
    proxy: { '/api': { target: 'http://127.0.0.1:8777', changeOrigin: true } },
  },
})
