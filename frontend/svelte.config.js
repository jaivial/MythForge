import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** Static SPA: nginx serves the built assets and proxies /api to the Rust API. */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({ fallback: 'index.html' }),
    alias: {
      $lib: 'src/lib',
      $components: 'src/lib/components'
    }
  }
};

export default config;
