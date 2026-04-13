import adapter from '@sveltejs/adapter-static'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

/** @type {import('@sveltejs/kit').Config} */
export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    runes: true,
  },
  kit: {
    files: {
      assets: 'public',
    },
    alias: {
      $backend: 'src/adapters/fumen-backend/src',
      $components: 'src/components',
    },
    adapter: adapter({
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html',
    }),
    prerender: {
      handleUnseenRoutes: 'ignore',
    },
  },
}
