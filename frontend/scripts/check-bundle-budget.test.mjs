import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { gzipSync } from 'node:zlib';
import { randomBytes } from 'node:crypto';
import {
  checkBundleBudget,
  extractInitialAssets,
  INITIAL_BUDGET_GZIP,
  PER_CHUNK_LIMIT_GZIP,
} from './check-bundle-budget.mjs';

let dir;

function makeFixture({ entry, preloads = [] }) {
  // entry/preloads: { name, content } — written under dist/assets/
  const assetsDir = join(dir, 'assets');
  mkdirSync(assetsDir, { recursive: true });
  const scriptTag = entry
    ? `<script type="module" crossorigin src="/assets/${entry.name}"></script>`
    : '';
  const preloadTags = preloads
    .map((p) => `<link rel="modulepreload" crossorigin href="/assets/${p.name}">`)
    .join('\n    ');
  writeFileSync(
    join(dir, 'index.html'),
    `<!doctype html><html><head>\n    ${preloadTags}\n    ${scriptTag}\n  </head><body></body></html>`,
  );
  for (const asset of [entry, ...preloads].filter(Boolean)) {
    writeFileSync(join(assetsDir, asset.name), asset.content);
  }
}

// Incompressible payload so gzip size ≈ raw size (Buffer.alloc(n, 1) would compress to nothing).
const big = (n) => randomBytes(n);
const small = Buffer.from('console.log("hi")');

beforeEach(() => {
  dir = mkdtempSync(join(tmpdir(), 'bundle-budget-'));
});
afterEach(() => {
  rmSync(dir, { recursive: true, force: true });
});

describe('extractInitialAssets', () => {
  it('picks up the entry script and modulepreload hrefs', () => {
    const html =
      '<script type="module" crossorigin src="/assets/index-a.js"></script>' +
      '<link rel="modulepreload" crossorigin href="/assets/vendor-b.js">' +
      '<link rel="stylesheet" href="/assets/index-a.css">';
    expect(extractInitialAssets(html)).toEqual(['/assets/index-a.js', '/assets/vendor-b.js']);
  });
});

describe('checkBundleBudget', () => {
  it('passes when under budget and under per-chunk limit', () => {
    makeFixture({
      entry: { name: 'index.js', content: small },
      preloads: [{ name: 'vendor-react.js', content: small }],
    });
    const result = checkBundleBudget(dir);
    expect(result.ok).toBe(true);
    expect(result.failures).toEqual([]);
  });

  it('fails when a modulepreloaded chunk exceeds the per-chunk limit', () => {
    makeFixture({
      entry: { name: 'index.js', content: small },
      preloads: [{ name: 'vendor.js', content: big(PER_CHUNK_LIMIT_GZIP * 4) }],
    });
    const result = checkBundleBudget(dir);
    expect(result.ok).toBe(false);
    expect(result.failures.some((f) => f.includes('per-chunk limit'))).toBe(true);
  });

  it('fails when the ENTRY alone exceeds the per-chunk limit (entries are not modulepreloaded)', () => {
    makeFixture({ entry: { name: 'index.js', content: big(PER_CHUNK_LIMIT_GZIP * 4) } });
    const result = checkBundleBudget(dir);
    expect(result.ok).toBe(false);
    expect(result.failures.some((f) => f.includes('index.js') && f.includes('per-chunk limit'))).toBe(true);
  });

  it('fails when the entry alone exceeds the total initial budget', () => {
    makeFixture({ entry: { name: 'index.js', content: big(INITIAL_BUDGET_GZIP * 2) } });
    const result = checkBundleBudget(dir, { perChunkLimit: Number.MAX_SAFE_INTEGER });
    expect(result.ok).toBe(false);
    expect(result.failures.some((f) => f.includes('initial JS gzip total'))).toBe(true);
  });

  it('fails when the summed initial set exceeds the total budget even if each chunk is fine', () => {
    const chunkSize = Math.floor(INITIAL_BUDGET_GZIP / 2);
    makeFixture({
      entry: { name: 'index.js', content: big(chunkSize * 2) },
      preloads: [{ name: 'a.js', content: big(chunkSize * 2) }],
    });
    const result = checkBundleBudget(dir, { perChunkLimit: Number.MAX_SAFE_INTEGER });
    const total = result.chunks.reduce((acc, c) => acc + c.gzip, 0);
    expect(total).toBeGreaterThan(INITIAL_BUDGET_GZIP);
    expect(result.ok).toBe(false);
    expect(result.failures.some((f) => f.includes('initial JS gzip total'))).toBe(true);
  });

  it('fails when index.html is missing', () => {
    const result = checkBundleBudget(dir);
    expect(result.ok).toBe(false);
    expect(result.failures[0]).toMatch(/index\.html not found/);
  });

  it('reports the measured gzip total it used for the decision', () => {
    makeFixture({ entry: { name: 'index.js', content: small } });
    const result = checkBundleBudget(dir);
    expect(result.initialGzip).toBe(gzipSync(small).length);
  });

  it('fails closed when index.html yields zero modulepreloads (extraction drifted)', () => {
    makeFixture({ entry: { name: 'index.js', content: small } });
    const result = checkBundleBudget(dir);
    expect(result.ok).toBe(false);
    expect(result.failures.some((f) => f.includes('modulepreload'))).toBe(true);
  });

  it('passes when at least one modulepreload exists and the vendor chunk is in the initial set', () => {
    makeFixture({
      entry: { name: 'index.js', content: small },
      preloads: [{ name: 'vendor-a1b2c3.js', content: small }],
    });
    const result = checkBundleBudget(dir);
    expect(result.ok).toBe(true);
  });

  it('fails when a vendor chunk exists in dist but is NOT in the initial set', () => {
    makeFixture({
      entry: { name: 'index.js', content: small },
      preloads: [{ name: 'vendor-other-x.js', content: small }],
    });
    // vendor present in dist/assets but absent from index.html
    writeFileSync(join(dir, 'assets', 'vendor-deadbeef.js'), small);
    const result = checkBundleBudget(dir);
    expect(result.ok).toBe(false);
    expect(result.failures.some((f) => f.includes('vendor'))).toBe(true);
  });

  it('fails when a modulepreloaded asset is missing from dist', () => {
    makeFixture({
      entry: { name: 'index.js', content: small },
      preloads: [{ name: 'ghost.js', content: small }],
    });
    rmSync(join(dir, 'assets', 'ghost.js'));
    const result = checkBundleBudget(dir);
    expect(result.ok).toBe(false);
    expect(result.failures.some((f) => f.includes('missing from dist'))).toBe(true);
  });

  it('dedupes an entry that is also modulepreloaded (counts once)', () => {
    const assetsDir = join(dir, 'assets');
    mkdirSync(assetsDir, { recursive: true });
    writeFileSync(
      join(dir, 'index.html'),
      '<link rel="modulepreload" crossorigin href="/assets/index.js">\n' +
        '<script type="module" crossorigin src="/assets/index.js"></script>',
    );
    writeFileSync(join(assetsDir, 'index.js'), small);
    const result = checkBundleBudget(dir);
    expect(result.initialGzip).toBe(gzipSync(small).length);
  });
});
