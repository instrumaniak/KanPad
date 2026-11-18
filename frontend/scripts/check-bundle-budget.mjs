#!/usr/bin/env node
/**
 * Bundle budget gate — run against `dist/` after `vite build`.
 *
 * Initial set = entry `<script type="module" src>` + every `<link rel="modulepreload">`.
 * The entry is NEVER modulepreloaded, so a modulepreload-only gate silently waives
 * the entry from every check — do not "simplify" this script that way.
 *
 * Fails (exit 1) when:
 *   - total gzip of the initial set > INITIAL_BUDGET_GZIP, or
 *   - any initial-load chunk > PER_CHUNK_LIMIT_GZIP, or
 *   - index.html yields ZERO modulepreload hrefs (vite always emits them for this
 *     app — zero means the extraction or the build config silently drifted and the
 *     gate would be vacuously green), or
 *   - a `vendor-*.js` chunk exists in dist/assets but is NOT in the initial
 *     set (the initial set must carry the app vendor code; a miss means the
 *     HTML/preload shape changed under us and the gate would undercount).
 *
 * Budgets are measure-then-set (T016, 2026-11-17, second scope amendment):
 * post-amendment initial gzip was 257,986 B (node zlib); T018 re-measured
 * 2026-11-18 at **258,443 B** (runtime 429 + vendor 188,065 + entry 69,949;
 * all app routes static; markdown-renderer and pele stay lazy; card-title
 * inline markdown is a dependency-free parser, so nothing from the markdown
 * stack is eager). Budget is that + ~19.9 % headroom so the gate guards
 * regression, not an absolute target.
 * Per-chunk limit covers the largest initial-load chunk (vendor, 188,065 B
 * gzip) + ~19.7 % headroom. First measurement (fully lazy routes) was
 * 167,385 B; the amendment re-set the numbers — see T016 Notes.
 *
 * Known scope boundary: lazy (`import()`) chunks are NOT bounded by this gate —
 * per-chunk and total budgets apply to the initial-load set only. A 2 MB lazy chunk
 * passes; a pathological lazy renderer leaking into a route is out of scope here.
 * Parses double-quoted attributes only (vite always emits double quotes); a document
 * with no extractable assets fails closed, never open.
 *
 * Usage: node scripts/check-bundle-budget.mjs [distDir]
 */
import { readFileSync, existsSync, readdirSync, realpathSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { gzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';

export const INITIAL_BUDGET_GZIP = 310_000; // measured 258,443 + ~19.9% headroom
export const PER_CHUNK_LIMIT_GZIP = 225_000; // vendor 188,065 + ~19.7% headroom

/** Extract the initial-load asset paths (site-root-absolute, e.g. /assets/x.js) from index.html. */
export function extractInitialAssets(indexHtml) {
  const assets = [];
  // entry: <script type="module" crossorigin src="/assets/index-xxx.js"></script>
  // (inline <script type="module"> without src is skipped — vite emits the entry with src)
  for (const m of indexHtml.matchAll(/<script\b[^>]*\btype="module"[^>]*>/g)) {
    const src = m[0].match(/\bsrc="([^"]+)"/);
    if (src) assets.push(src[1]);
  }
  // modulepreload: <link rel="modulepreload" crossorigin href="/assets/x.js">
  for (const m of indexHtml.matchAll(/<link\b[^>]*\brel="modulepreload"[^>]*>/g)) {
    const href = m[0].match(/\bhref="([^"]+)"/);
    if (href) assets.push(href[1]);
  }
  return [...new Set(assets)]; // dedupe: an entry listed as its own preload must not double-count
}

/**
 * Check a built dist directory against the budgets.
 * Returns { ok, initialGzip, chunks: [{asset, gzip}], failures: string[] }.
 */
export function checkBundleBudget(distDir, { initialBudget = INITIAL_BUDGET_GZIP, perChunkLimit = PER_CHUNK_LIMIT_GZIP } = {}) {
  const indexPath = join(distDir, 'index.html');
  if (!existsSync(indexPath)) {
    return { ok: false, initialGzip: 0, chunks: [], failures: [`index.html not found at ${indexPath}`] };
  }
  const indexHtml = readFileSync(indexPath, 'utf8');
  const preloadCount = [...indexHtml.matchAll(/<link\b[^>]*\brel="modulepreload"[^>]*>/g)].length;
  const assets = extractInitialAssets(indexHtml);
  if (assets.length === 0) {
    return { ok: false, initialGzip: 0, chunks: [], failures: ['no initial-load assets found in index.html (entry script or modulepreload)'] };
  }

  const chunks = [];
  const failures = [];
  let initialGzip = 0;
  for (const asset of assets) {
    const rel = asset.replace(/^\//, '');
    const filePath = join(distDir, rel);
    if (!existsSync(filePath)) {
      failures.push(`initial-load asset missing from dist: ${asset}`);
      continue;
    }
    const gzip = gzipSync(readFileSync(filePath)).length;
    chunks.push({ asset, gzip });
    initialGzip += gzip;
    if (gzip > perChunkLimit) {
      failures.push(`${asset}: ${gzip} B gzip exceeds per-chunk limit of ${perChunkLimit} B`);
    }
  }
  if (initialGzip > initialBudget) {
    failures.push(`initial JS gzip total ${initialGzip} B exceeds budget of ${initialBudget} B`);
  }
  // Drift tripwires: vite always emits modulepreloads + a vendor chunk for this
  // app. Zero preloads (or the vendor chunk missing from the initial set) means
  // the HTML/preload shape changed and every number above is undercounting — fail,
  // don't shrug.
  if (preloadCount === 0) {
    failures.push('no modulepreload hrefs found in dist/index.html — extraction or build config drifted; gate would undercount');
  }
  const assetsDir = join(distDir, 'assets');
  if (existsSync(assetsDir)) {
    const initialNames = new Set(chunks.map((c) => c.asset.split('/').pop()));
    const vendorChunk = readdirSync(assetsDir).find((f) => /^vendor-.*\.js$/.test(f));
    if (vendorChunk && !initialNames.has(vendorChunk)) {
      failures.push(`vendor chunk ${vendorChunk} exists in dist/assets but is NOT in the initial set — initial-load extraction drifted; gate would undercount`);
    }
  }
  return { ok: failures.length === 0, initialGzip, chunks, failures };
}

let isMain = false;
try {
  isMain = !!process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
} catch {
  isMain = false;
}
if (isMain) {
  const distDir = resolve(process.argv[2] ?? 'dist');
  const { ok, initialGzip, chunks, failures } = checkBundleBudget(distDir);
  console.log('Initial-load chunks (entry <script type="module" src> + modulepreload):');
  for (const { asset, gzip } of [...chunks].sort((a, b) => b.gzip - a.gzip)) {
    console.log(`  ${String(gzip).padStart(8)} B  ${asset}`);
  }
  console.log(`  ${String(initialGzip).padStart(8)} B  TOTAL initial gzip (budget: ${INITIAL_BUDGET_GZIP} B)`);
  if (!ok) {
    console.error('\nBundle budget FAILED:');
    for (const f of failures) console.error(`  - ${f}`);
    process.exit(1);
  }
  console.log('\nBundle budget OK');
}
