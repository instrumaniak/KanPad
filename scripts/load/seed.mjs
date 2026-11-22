// B3 scalable seed for k6 load runs. Sessions via the login API.
// Mechanical prod-guards: NO dotenv autoload (repo-root .env holds live secrets).
// Refuses non-throwaway targets.

import { parseArgs } from 'node:util';

const { values } = parseArgs({
  options: {
    boards: { type: 'string', default: '10' },
    cards: { type: 'string', default: '100' },
    baseUrl: { type: 'string', default: process.env.BASE_URL || 'http://localhost:5173' },
    email: { type: 'string', default: 'load@test.local' },
    password: { type: 'string', default: 'Test1234!' },
  },
});

const BASE = values.baseUrl.replace(/\/$/, '');

// BASE_URL guard: refuse non-loopback targets unless explicitly allowed.
{
  const u = new URL(BASE);
  const loopbackHost = u.hostname === 'localhost' || u.hostname === '127.0.0.1' || u.hostname === '[::1]';
  if (!loopbackHost && process.env.K6_ALLOW_REMOTE !== '1') {
    console.error(
      `Refusing to seed: BASE_URL host "${u.hostname}" is not loopback. Set K6_ALLOW_REMOTE=1 to override (never for prod).`,
    );
    process.exit(1);
  }
}

const DB_NAME = process.env.DB_NAME || process.env.MYSQL_DATABASE || '';
const DB_HOST = process.env.DB_HOST || '127.0.0.1';

// Throwaway DB guard (when explicitly provided)
if (DB_NAME && !/^kanpad_(e2e|load|test)/.test(DB_NAME)) {
  console.error(`Refusing to seed: DB name "${DB_NAME}" is not a throwaway (kanpad_e2e/load/test).`);
  process.exit(1);
}
const loopbackDbHost = ['127.0.0.1', 'localhost', 'mysql', 'mariadb', '[::1]'].includes(DB_HOST);
if (DB_HOST && !loopbackDbHost) {
  console.error(`Refusing to seed: DB host "${DB_HOST}" is not loopback/compose.`);
  process.exit(1);
}

const N = Number(values.boards);
const M = Number(values.cards);

async function req(method, path, body, cookie) {
  const headers = { 'Content-Type': 'application/json' };
  if (cookie) headers.Cookie = cookie;
  const res = await fetch(BASE + path, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let json;
  try { json = JSON.parse(text); } catch { json = null; }
  // capture set-cookie
  const setCookie = res.headers.get('set-cookie');
  return { status: res.status, json, setCookie };
}

// register (idempotent)
let cookie;
const reg = await req('POST', '/api/v1/auth/register', { email: values.email, password: values.password });
if (reg.status !== 201 && reg.status !== 409) {
  console.error('register failed', reg.status, reg.json);
  process.exit(1);
}
const login = await req('POST', '/api/v1/auth/login', { email: values.email, password: values.password });
if (login.status !== 200) {
  console.error('login failed', login.status, login.json);
  process.exit(1);
}
cookie = (login.setCookie || '').split(';')[0];

console.log(`Seeding ${N} boards x ${M} cards for ${values.email}...`);
for (let b = 0; b < N; b++) {
  const br = await req('POST', '/api/v1/boards', { name: `load-board-${b}-${Date.now()}` }, cookie);
  if (br.status !== 201) {
    console.error('board create failed', br.status, br.json);
    process.exit(1);
  }
  const boardId = br.json?.data?.id ?? br.json?.id;
  // first column
  const cols = await req('GET', `/api/v1/boards/${boardId}/columns`, null, cookie);
  const columnId = (cols.json?.data ?? cols.json)?.[0]?.id;
  for (let c = 0; c < M; c++) {
    await req('POST', '/api/v1/cards', { title: `card-${b}-${c}`, column_id: columnId }, cookie);
  }
  console.log(`  board ${b + 1}/${N} done`);
}
console.log('Seed complete.');
