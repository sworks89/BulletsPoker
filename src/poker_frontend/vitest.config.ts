import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Separate from vite.config.ts, whose dev-server setup shells out to `icp`.
export default defineConfig({
  plugins: [react()],
  // styles.css is read as text by phone.test.ts; other stylesheets stay stubbed out.
  test: { environment: 'node', css: { include: [/\/styles\.css(\?|$)/] } },
});
