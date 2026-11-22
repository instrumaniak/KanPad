// Minimal registry schema check for docs/perf-budgets.md (T046).
import fs from 'node:fs';
const text = fs.readFileSync('docs/perf-budgets.md', 'utf8');
const lines = text.split('\n');
const headerIdx = lines.findIndex((l) => l.includes('| # | Name |'));
if (headerIdx === -1) { console.error('registry table not found'); process.exit(1); }
// parse header
const header = lines[headerIdx].split('|').map((s) => s.trim()).filter(Boolean);
const required = ['Value', 'Tier', 'Enforcing test', 'Last measured', 'NFR mapping'];
const DATE_PREFIX = /^\d{4}-\d{2}-\d{2}/;
const LABELS = new Set(['—', 'none', 'deferred/unenforced', 'pending']);
for (const r of required) {
  if (!header.includes(r)) { console.error(`missing column: ${r}`); process.exit(1); }
}
// data rows until blank/next heading
let rows = 0;
for (let i = headerIdx + 2; i < lines.length; i++) {
  const line = lines[i].trim();
  if (!line.startsWith('|')) break;
  const cells = line.split('|').map((s) => s.trim()).filter(Boolean);
  if (cells.length < header.length) { console.error(`malformed row: ${line}`); process.exit(1); }
  // check required fields non-empty
  for (const col of required) {
    const idx = header.indexOf(col);
    if (!cells[idx]) { console.error(`empty ${col} in row: ${line}`); process.exit(1); }
  }
  // Last measured: must be a date (freshness rule — same-PR updates on
  // change) or an explicit label for rows that are explicitly unenforcing
  // (spec: "last-measured date" + explicit "not NFR-enforcing" label).
  {
    const lmIdx = header.indexOf('Last measured');
    const lm = cells[lmIdx];
    // Date prefix (freshness) or an explicit label prefix for rows that are
    // explicitly unenforcing or unmeasured (spec: "last-measured date" +
    // explicit "not NFR-enforcing" label).
    if (!DATE_PREFIX.test(lm) && ![...LABELS].some((l) => lm.startsWith(l))) {
      console.error(`Last measured not a date or explicit label in row: ${line}`);
      process.exit(1);
    }
  }
  rows++;
}
if (rows === 0) { console.error('no registry rows'); process.exit(1); }
console.log(`registry schema OK (${rows} rows)`);
