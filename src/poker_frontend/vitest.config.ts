import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Separate from vite.config.ts, whose dev-server setup shells out to `icp`.
export default defineConfig({
  plugins: [react()],
  test: { environment: 'node' },
});
