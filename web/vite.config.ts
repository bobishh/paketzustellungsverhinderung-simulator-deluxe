import { defineConfig } from 'vite';
const apiOrigin = process.env.PAKET_API_ORIGIN || 'http://127.0.0.1:8788';
export default defineConfig({
  build: { target: 'esnext' },
  server: {
    port: 4188, strictPort: true,
    proxy: { '/api': {
      target: apiOrigin, changeOrigin: true,
      configure(proxy) {
        proxy.on('proxyReq', (outgoing, incoming) => {
          // Preserve the same-origin boundary across the local development proxy.
          if (incoming.headers.origin && new URL(incoming.headers.origin).host === incoming.headers.host) {
            outgoing.setHeader('origin', apiOrigin);
          }
        });
      },
    } },
  },
});
