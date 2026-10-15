import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const base = process.env.PAGES_BASE_PATH || '/mathesis/';
if (!/^\/(?:[a-zA-Z0-9_-]+\/)*$/.test(base)) {
  throw new Error('PAGES_BASE_PATH must be an absolute directory path, such as /mathesis/ or /.');
}
const application = process.env.VITE_MATHESIS_APP_URL;
if (application) {
  const url = new URL(application);
  if (url.protocol !== 'https:' || url.username || url.password) {
    throw new Error('VITE_MATHESIS_APP_URL must be an HTTPS URL without credentials.');
  }
}

export default defineConfig({
  root: fileURLToPath(new URL('./showcase', import.meta.url)),
  envDir: fileURLToPath(new URL('.', import.meta.url)),
  publicDir: false,
  base,
  build: { outDir: '../dist-showcase', emptyOutDir: true },
  server: { host: '127.0.0.1' },
  preview: { host: '127.0.0.1' },
});
