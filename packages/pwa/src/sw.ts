/**
 * Service Worker (FE-pwa-7) — offline shell only
 * Cache-first for app shell, network-first for WS tunnel
 * No push notifications (L2+)
 */

/// <reference lib="webworker" />

declare const self: ServiceWorkerGlobalScope

const CACHE_NAME = 'telayd-v1'
const SHELL_ASSETS = [
  '/',
  '/index.html',
]

self.addEventListener('install', (event: ExtendableEvent) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {
      return cache.addAll(SHELL_ASSETS)
    })
  )
  // Activate immediately
  self.skipWaiting()
})

self.addEventListener('activate', (event: ExtendableEvent) => {
  // Clean up old caches
  event.waitUntil(
    caches.keys().then((names) =>
      Promise.all(
        names
          .filter((name) => name !== CACHE_NAME)
          .map((name) => caches.delete(name))
      )
    )
  )
  self.clients.claim()
})

self.addEventListener('fetch', (event: FetchEvent) => {
  const { request } = event
  const url = new URL(request.url)

  // Skip non-GET and WS requests
  if (request.method !== 'GET') return
  if (url.protocol === 'ws:' || url.protocol === 'wss:') return

  // Skip trycloudflare.com API calls — always network
  if (url.hostname.endsWith('.trycloudflare.com')) return

  event.respondWith(
    caches.match(request).then((cached) => {
      if (cached) return cached
      return fetch(request).then((response) => {
        // Cache successful same-origin responses
        if (response.ok && url.origin === self.location.origin) {
          const clone = response.clone()
          caches.open(CACHE_NAME).then((cache) => cache.put(request, clone))
        }
        return response
      })
    })
  )
})
