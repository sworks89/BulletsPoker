import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// `dfx deploy` writes canister ids to the repo-root .env (see dfx.json).
export default defineConfig({
  plugins: [react()],
  envDir: '../..',
  envPrefix: ['VITE_', 'CANISTER_', 'DFX_'],
  build: { outDir: 'dist', emptyOutDir: true },
  server: {
    proxy: { '/api': { target: 'http://127.0.0.1:4943', changeOrigin: true } },
  },
});
