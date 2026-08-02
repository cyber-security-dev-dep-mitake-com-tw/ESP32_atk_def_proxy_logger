import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The dev server proxies API + WebSocket calls to the Go agent on :8080 so the
// browser can talk to a single origin during development.
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      '/api': 'http://localhost:8080',
      '/ws': { target: 'ws://localhost:8080', ws: true },
    },
  },
})
