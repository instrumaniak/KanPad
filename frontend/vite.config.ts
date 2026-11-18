import { defineConfig } from 'vite'
import react, { reactCompilerPreset } from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import babel from '@rolldown/plugin-babel'
import path from 'path'

// Production node_modules whose code must stay OFF the initial path: the
// react-markdown/unified/pele family is only reachable from the lazy
// markdown-renderer (and pele/lazy) — nothing eagerly imports it (the card
// face title uses the dependency-free inline renderer in markdown-preview.tsx).
// They are excluded from the catch-all `vendor` group so vendor stays
// initial-but-lean; rolldown then splits the markdown path naturally
// (one lazy entry → few chunks). Verified by prod-graph closure
// (npm ls --omit=dev): every entry below has zero out-of-family prod
// consumers in this app's dependency graph.
// RE-DERIVE this set (prod closure, cross-family consumers carved out) on every
// markdown/unified/pele dependency add or upgrade — a stale set doesn't break
// correctness (the budget gate still trips past 310 KB) but quietly grows the
// initial path.
const LAZY_ONLY_PACKAGES = new Set([
  '@ungap/structured-clone', 'bail', 'ccount', 'character-entities',
  'character-entities-html4', 'character-entities-legacy', 'character-reference-invalid',
  'comma-separated-tokens', 'debug', 'decode-named-character-reference', 'dequal', 'devlop',
  'entities', 'escape-string-regexp', 'estree-util-is-identifier-name', 'extend',
  'hast-util-from-parse5', 'hast-util-is-element', 'hast-util-parse-selector', 'hast-util-raw',
  'hast-util-sanitize', 'hast-util-to-jsx-runtime', 'hast-util-to-parse5', 'hast-util-to-text',
  'hast-util-whitespace', 'hastscript', 'highlight.js', 'html-url-attributes',
  'html-void-elements', 'inline-style-parser', 'is-alphabetical', 'is-alphanumerical',
  'is-decimal', 'is-hexadecimal', 'is-plain-obj',
  'longest-streak', 'lowlight', 'markdown-table', 'mdast-util-find-and-replace',
  'mdast-util-from-markdown', 'mdast-util-gfm', 'mdast-util-gfm-autolink-literal',
  'mdast-util-gfm-footnote', 'mdast-util-gfm-strikethrough', 'mdast-util-gfm-table',
  'mdast-util-gfm-task-list-item', 'mdast-util-mdx-expression', 'mdast-util-mdx-jsx',
  'mdast-util-mdxjs-esm', 'mdast-util-phrasing', 'mdast-util-to-hast', 'mdast-util-to-markdown',
  'mdast-util-to-string', 'micromark', 'micromark-core-commonmark', 'micromark-extension-gfm',
  'micromark-extension-gfm-autolink-literal', 'micromark-extension-gfm-footnote',
  'micromark-extension-gfm-strikethrough', 'micromark-extension-gfm-table',
  'micromark-extension-gfm-tagfilter', 'micromark-extension-gfm-task-list-item',
  'micromark-factory-destination', 'micromark-factory-label', 'micromark-factory-space',
  'micromark-factory-title', 'micromark-factory-whitespace', 'micromark-util-character',
  'micromark-util-chunked', 'micromark-util-classify-character', 'micromark-util-combine-extensions',
  'micromark-util-decode-numeric-character-reference', 'micromark-util-decode-string',
  'micromark-util-encode', 'micromark-util-html-tag-name', 'micromark-util-normalize-identifier',
  'micromark-util-resolve-all', 'micromark-util-sanitize-uri', 'micromark-util-subtokenize',
  'micromark-util-symbol', 'micromark-util-types', 'ms', 'parse-entities', 'parse5',
  'dompurify', 'pele', 'property-information', 'react-markdown', 'rehype-highlight', 'rehype-raw',
  'rehype-sanitize', 'remark-gfm', 'remark-parse', 'remark-rehype', 'remark-stringify',
  'space-separated-tokens', 'stringify-entities', 'style-to-js', 'style-to-object',
  'trim-lines', 'trough', 'unified', 'unist-util-find-after', 'unist-util-is',
  'unist-util-position', 'unist-util-stringify-position', 'unist-util-visit',
  'unist-util-visit-parents', 'vfile', 'vfile-location', 'vfile-message', 'web-namespaces',
  'zwitch',
]);

/** Package name (incl. scope) of a module id, or null if not under node_modules.
 *  Uses the LAST node_modules segment so nested installs (e.g.
 *  hast-util-raw/node_modules/parse5) attribute to parse5, not hast-util-raw. */
function nodeModulePkg(id: string): string | null {
  const segments = id.split(/node_modules[\\/]/);
  const tail = segments[segments.length - 1];
  if (segments.length < 2 || !tail) return null;
  const parts = tail.split(/[\\/]/);
  if (parts[0]?.startsWith('@') && parts[1]) return `${parts[0]}/${parts[1]}`;
  return parts[0] ?? null;
}

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
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
  server: {
    proxy: {
      // T01: VITE_API_URL selects the backend target (Rust, :3001).
      // The in-app fetch BASE switch itself lives in client.ts (T40b).
      '/api': process.env.VITE_API_URL ?? 'http://localhost:3001',
    },
  },
  build: {
    chunkSizeWarningLimit: 700,
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              // Everything node_modules EXCEPT the markdown/pele lazy-only set
              // above → one long-cacheable vendor chunk (initial path).
              test: (id: string) => {
                const pkg = nodeModulePkg(id);
                return pkg !== null && !LAZY_ONLY_PACKAGES.has(pkg);
              },
              name: 'vendor',
              priority: 10,
            },
          ],
        },
      },
    },
  },
})
