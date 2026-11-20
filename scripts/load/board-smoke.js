// k6 load scenario skeleton (T046 B3). Tuned by T026.
// Usage: k6 run scripts/load/board-smoke.js

import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE = __ENV.BASE_URL || 'http://localhost:5173';

export const options = {
  stages: [
    { duration: '1m', target: 50 },
    { duration: '1m', target: 100 },
    { duration: '5m', target: 100 },
    { duration: '2m', target: 0 },
  ],
  // ramp 0->50->100 over 2m approximated by stages above; adjust in T026
};

// Cookie must be sent explicitly (Secure cookies dropped over plain HTTP by Go cookiejar).
const COOKIE = __ENV.COOKIE || '';

export default function () {
  const headers = { Cookie: COOKIE };
  const boards = http.get(`${BASE}/api/v1/boards`, { headers });
  check(boards, { 'boards 200': (r) => r.status === 200 });
  sleep(1);
}
