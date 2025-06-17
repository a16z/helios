import { defineConfig } from 'vite'

export default defineConfig({
  build: {
    target: 'es2020',
    minify: true,
    sourcemap: true
  },
  server: {
    port: 3000
  }
})