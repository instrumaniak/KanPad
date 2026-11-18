/**
 * Centralized API client (T40b).
 *
 * Unifies the three `handleResponse` variants previously copy-pasted across
 * the nine `features/*\/ *.api.ts` modules:
 *  1. standard (`boards/projects/cards/columns/tags/notes/labels`):
 *     `!ok` → parse `error.message` → throw; success → `response.json()`
 *     throwing `Unexpected response format` on empty bodies;
 *  2. `auth.api.ts` inline-fetch: per-function error blocks, `loginApi`
 *     wrapping `fetch` rejections into `Network error`, `logoutApi` bare
 *     `Promise<void>`;
 *  3. `checklists.api.ts` divergent: `errorBody.message || 'Request failed
 *     with status …'`, explicit `204 → undefined`, void deletes.
 *
 * Notes on the unification choices:
 * - `204 → undefined as T` (variant 3's explicit path) applies to every
 *   endpoint here, not just labels/checklists — the Rust rewrite returns
 *   `204` for all deletes/logout, and every other module today throws
 *   `Unexpected response format` on an empty body.
 * - `fetch` rejections (network failures) propagate unchanged as `TypeError`
 *   so `getErrorMessage`'s `Failed to fetch` branch keeps working; callers
 *   that need a custom message catch the `TypeError` themselves.
 * - Error bodies accept BOTH the legacy Nest shape
 *   `{message: string | string[]}` AND the RFC9457 problem+json shape
 *   `{title, detail, errors}`. The thrown `ApiError` always preserves
 *   `status`, which is what makes `getErrorMessage`'s status-routing branch
 *   live (it already reads `error.status` — no rewire needed).
 */

/**
 * In-flight requests are aborted after this delay and rejected with a typed
 * {@link TimeoutError}. 15 s ≈ 30× the API p95 NFR (<500 ms,
 * `specs/arch-hardening-specs/20-performance.md`) — generous headroom for
 * legitimate cold paths (DB lock waits, slow VPS I/O) while still bounding a
 * hung connection so it surfaces as an error instead of a forever-spinner.
 * Exported so tests and future callers can reference the default.
 */
export const DEFAULT_TIMEOUT_MS = 15_000;

/**
 * Typed error for «this request exceeded DEFAULT_TIMEOUT_MS». Distinct from
 * `AbortError` (caller-cancelled) and from `ApiError` (server responded with
 * a non-2xx) so the retry predicate and `getErrorMessage` can route it.
 */
export class TimeoutError extends Error {
  constructor(message = 'Network timeout — check your connection') {
    super(message);
    this.name = 'TimeoutError';
  }
}

export interface ApiResponse<T> {
  data: T;
  message?: string;
}

export interface Page<T> {
  data: T[];
  page: number;
  limit: number;
  total: number;
}

export class ApiError extends Error {
  status: number;
  title?: string;
  detail?: string;
  errors?: unknown;

  constructor(
    status: number,
    message: string,
    opts?: { title?: string; detail?: string; errors?: unknown },
  ) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.title = opts?.title;
    this.detail = opts?.detail;
    this.errors = opts?.errors;
  }
}

/**
 * Backend origin switch. Default stays relative so the Vite dev proxy keeps
 * working; setting `VITE_API_URL` points the frontend at an absolute API origin
 * (`:3001` Rust in dev; the two-origin prod path). Owned here (T40b), not in
 * `vite.config.ts`.
 */
export const BASE: string = import.meta.env.VITE_API_URL ?? '';

const FETCH_OPTIONS: RequestInit = {
  credentials: 'include',
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function toApiError(status: number, statusText: string, body: unknown): ApiError {
  const fallback = statusText || `Request failed with status ${status}`;
  if (!isRecord(body)) {
    return new ApiError(status, fallback);
  }
  // Legacy Nest shape: { message: string | string[] }.
  const message = body['message'];
  if (typeof message === 'string' && message.length > 0) {
    return new ApiError(status, message);
  }
  if (Array.isArray(message)) {
    const joined = message.filter((m): m is string => typeof m === 'string').join(', ');
    if (joined.length > 0) {
      return new ApiError(status, joined);
    }
  }
  // RFC9457 problem+json shape: { title, detail, errors }.
  if ('title' in body || 'detail' in body || 'errors' in body) {
    const title = typeof body['title'] === 'string' ? body['title'] : undefined;
    const detail = typeof body['detail'] === 'string' ? body['detail'] : undefined;
    return new ApiError(status, detail ?? title ?? fallback, {
      title,
      detail,
      errors: body['errors'],
    });
  }
  return new ApiError(status, fallback);
}

async function parseFailure(response: Response): Promise<ApiError> {
  let body: unknown;
  try {
    body = await response.json();
  } catch {
    body = undefined;
  }
  return toApiError(response.status, response.statusText, body);
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  if (init?.body !== undefined && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  const callerSignal = init?.signal ?? undefined;
  const controller = new AbortController();
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, DEFAULT_TIMEOUT_MS);
  // Chain a caller-supplied signal: aborting it aborts OUR controller, so the
  // fetch still stops — but the catch below distinguishes the two causes and
  // only our own timeout is re-labelled as a TimeoutError.
  const onCallerAbort = () => controller.abort();
  if (callerSignal) {
    if (callerSignal.aborted) {
      controller.abort();
    } else {
      callerSignal.addEventListener('abort', onCallerAbort, { once: true });
    }
  }
  try {
    const response = await fetch(`${BASE}${path}`, {
      ...FETCH_OPTIONS,
      ...init,
      signal: controller.signal,
      headers,
    });
    if (!response.ok) {
      throw await parseFailure(response);
    }
    if (response.status === 204) {
      return undefined as T;
    }
    try {
      return (await response.json()) as T;
    } catch {
      throw new ApiError(response.status, 'Unexpected response format');
    }
  } catch (err) {
    // A caller-initiated abort always wins (explicit user action); only an
    // abort originating from OUR timer is re-labelled as a TimeoutError.
    if (timedOut && !callerSignal?.aborted) {
      throw new TimeoutError();
    }
    throw err;
  } finally {
    clearTimeout(timer);
    callerSignal?.removeEventListener('abort', onCallerAbort);
  }
}

/** Void-returning sibling for `204` endpoints (deletes, logout). */
export async function apiFetchVoid(path: string, init?: RequestInit): Promise<void> {
  await apiFetch<unknown>(path, init);
}
