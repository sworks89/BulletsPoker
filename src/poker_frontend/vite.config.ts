import { execSync } from 'node:child_process';
import { defineConfig, type ServerOptions } from 'vite';
import react from '@vitejs/plugin-react';

// In production the frontend canister sets the `ic_env` cookie (canister ids and
// the network's root key). The dev server simulates it from the running local
// network, so `icp network start -d && icp deploy poker_backend` must come first.
function devServerConfig(): ServerOptions {
  const environment = process.env.ICP_ENVIRONMENT || 'local';
  const icp = (args: string) => execSync(`icp ${args}`, { encoding: 'utf-8', stdio: 'pipe' }).trim();
  const network = JSON.parse(icp(`network status -e ${environment} --json`));
  const backendId = icp(`canister status poker_backend -e ${environment} --id-only`);
  const cookie = `PUBLIC_CANISTER_ID:poker_backend=${backendId}&ic_root_key=${network.root_key}`;
  return {
    headers: { 'Set-Cookie': `ic_env=${encodeURIComponent(cookie)}; SameSite=Lax;` },
    proxy: { '/api': { target: network.api_url, changeOrigin: true } },
  };
}

export default defineConfig(({ command }) => ({
  plugins: [react()],
  build: { outDir: 'dist', emptyOutDir: true },
  ...(command === 'serve' ? { server: devServerConfig() } : {}),
}));
