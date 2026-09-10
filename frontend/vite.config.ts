import { defineConfig } from 'vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import babel from '@rolldown/plugin-babel'
import path from 'path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    babel({
      presets: [reactCompilerPreset()],
    }),
    tailwindcss(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3000',
    },
  },
  build: {
    chunkSizeWarningLimit: 800,
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              test: /node_modules[\\/]mermaid/,
              name: 'vendor-mermaid',
              priority: 30,
            },
            {
              test: /node_modules[\\/]react(?:-dom|-compiler)?/,
              name: 'vendor-react',
              priority: 20,
            },
            {
              test: /node_modules/,
              name: 'vendor',
              minSize: 50000,
              priority: 10,
            },
            {
              name: 'app',
              minSize: 100000,
              priority: 1,
            },
          ],
        },
      },
    },
  },
})
