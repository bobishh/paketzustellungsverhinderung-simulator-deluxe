import { defineConfig } from 'vite';
export default defineConfig({ build: { target: 'esnext' }, server: { port: 4188, strictPort: true, proxy: { '/api': 'http://127.0.0.1:8788' } } });
