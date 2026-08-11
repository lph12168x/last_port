import { defineConfig, type UserConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { fileURLToPath, URL } from 'node:url';

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
const config: UserConfig = {
  plugins: [svelte()],
  clearScreen: false,
  // $lib 别名,组件通过 `$lib/types` `$lib/api` 等访问
  resolve: {
    alias: {
      $lib: fileURLToPath(new URL('./src/lib', import.meta.url))
    }
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: {
      // 不监听 Rust 源码变化（由 cargo 自己处理）
      ignored: ['**/src-tauri/**']
    }
  },
  // 暴露给前端的 env 变量前缀
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    target: 'es2022',
    minify: process.env.TAURI_DEBUG ? false : 'esbuild',
    sourcemap: !!process.env.TAURI_DEBUG
  }
};

export default defineConfig(config);