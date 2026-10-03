import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    outDir: 'public/worker',
    rollupOptions: {
      input: './src/worker/worker.ts',
      output: {
        format: 'esm',
        entryFileNames: 'worker.js',
        chunkFileNames: '[name].js',
        assetFileNames: 'assets/[name].[ext]',
      },
    },
    emptyOutDir: true,
    minify: 'esbuild',
    target: 'esnext',
  },
});