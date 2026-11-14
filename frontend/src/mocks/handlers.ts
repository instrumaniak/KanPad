import { http, HttpResponse } from 'msw';
import type { components } from '@/lib/api/schema';

/**
 * Shared baseline handlers for Storybook (and any manual `setupWorker` use).
 * Typed against the generated OpenAPI schema so a contract change breaks the
 * build instead of silently drifting. Per-story wiring belongs in
 * `beforeEach({ msw }) { msw.use(...handlers) }` (CSF 3) — `parameters.msw`
 * is deprecated in msw-storybook-addon v3, and `msw` is the `SetupWorker`.
 */
type PageTagResponse = components['schemas']['Page_TagResponse'];

export const handlers = [
  http.get('/api/v1/tags', () =>
    HttpResponse.json({
      data: [
        { id: 1, name: 'Bug', color: 'teal', created_at: '2026-01-01T00:00:00.000000Z' },
        { id: 2, name: 'Feature', color: 'blue', created_at: '2026-01-01T00:00:00.000000Z' },
      ],
      page: 1,
      limit: 50,
      total: 2,
    } satisfies PageTagResponse),
  ),
];
