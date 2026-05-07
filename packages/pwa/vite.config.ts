import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import type { Plugin } from 'vite'
import { VitePWA } from 'vite-plugin-pwa'

/**
 * IG11: strip ws://localhost:7777 from the CSP connect-src in production builds.
 * The dev server serves to localhost where the daemon is reachable directly;
 * production traffic always goes through the cloudflared wss:// tunnel only.
 */
function stripLocalWsFromCsp(): Plugin {
  return {
    name: 'strip-local-ws-csp',
    apply: 'build',
    transformIndexHtml(html) {
      return html.replace(/ ws:\/\/localhost:7777/g, '')
    },
  }
}

export default defineConfig({
  plugins: [
    react(),
    stripLocalWsFromCsp(),
    VitePWA({
      registerType: 'autoUpdate',
      includeAssets: ['favicon.svg'],
      manifest: false, // manifest is in public/manifest.webmanifest
      workbox: {
        globPatterns: ['**/*.{js,css,html,svg,webmanifest}'],
        // IG8: remove trycloudflare.com runtimeCaching — WS-only, no GET cache needed.
        // Caching WS-upgrade responses is harmful and causes stale-cache hits on URL rotation.
        runtimeCaching: [],
      },
    }),
  ],
  server: {
    port: 5173,
    host: true,
  },
  build: {
    target: 'es2020',
    outDir: 'dist',
    // IG8: hidden sourcemap for production — avoids exposing full source tree via CF tunnel.
    // Use 'inline' in dev via vite server (already default for dev mode).
    sourcemap: 'hidden',
  },
})
