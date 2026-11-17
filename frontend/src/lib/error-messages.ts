type ErrorType = 'network' | 'timeout' | 'server' | 'validation' | 'unauthorized' | 'unknown';

interface ErrorMessage {
  title: string;
  message: string;
  retry: boolean;
  redirect?: string;
}

export const ERROR_MESSAGES: Record<ErrorType, ErrorMessage> = {
  network: {
    title: 'Connection Error',
    message: 'Unable to reach the server. Check your internet connection.',
    retry: true,
  },
  timeout: {
    title: 'Request Timeout',
    message: 'The request took too long. Please try again.',
    retry: true,
  },
  server: {
    title: 'Server Error',
    message: 'Something went wrong on our end. Please try again later.',
    retry: true,
  },
  validation: {
    title: 'Invalid Input',
    message: 'Please check your input and try again.',
    retry: false,
  },
  unauthorized: {
    title: 'Session Expired',
    message: 'Your session has expired. Please log in again.',
    retry: false,
    redirect: '/login',
  },
  unknown: {
    title: 'Something Went Wrong',
    message: 'An unexpected error occurred. Please try again.',
    retry: true,
  },
};

// Browser error text that must never reach the user verbatim.
const BROWSER_JARGON = /failed to fetch|load failed|networkerror|network request failed/i;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function pickMessage(text: unknown, fallback: string): string {
  if (typeof text === 'string' && text.trim().length > 0 && !BROWSER_JARGON.test(text)) {
    return text;
  }
  return fallback;
}

function messageOf(error: unknown): unknown {
  return isRecord(error) ? error['message'] : undefined;
}

function statusOf(error: unknown): number | undefined {
  if (!isRecord(error)) {
    return undefined;
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const status = (error as any)?.response?.status ?? (error as any)?.status;
  return typeof status === 'number' ? status : undefined;
}

function withMessage(base: ErrorMessage, error: unknown): ErrorMessage {
  return { ...base, message: pickMessage(messageOf(error), base.message) };
}

/**
 * Single owner for user-facing error text. Taxonomy owns `title`/`retry`/
 * `redirect`; the server's own (human) message wins for `message` whenever
 * present, falling back to the taxonomy text otherwise.
 */
export function getErrorMessage(error: unknown): ErrorMessage {
  if (error instanceof TypeError && BROWSER_JARGON.test(error.message)) {
    // Browser network failures (Chrome: Failed to fetch; Firefox: NetworkError…;
    // Safari: Load failed) — all get the network taxonomy.
    return { ...ERROR_MESSAGES.network };
  }
  if (isRecord(error) && error['name'] === 'TimeoutError') {
    // Typed timeout from apiFetch — its message is already user-friendly.
    return withMessage(ERROR_MESSAGES.timeout, error);
  }
  if (isRecord(error) && error['name'] === 'AbortError') {
    return { ...ERROR_MESSAGES.timeout };
  }
  const status = statusOf(error);
  if (status !== undefined) {
    if (status === 401) return withMessage(ERROR_MESSAGES.unauthorized, error);
    if (status >= 500) return withMessage(ERROR_MESSAGES.server, error);
    if (status >= 400) return withMessage(ERROR_MESSAGES.validation, error);
  }
  return withMessage(ERROR_MESSAGES.unknown, error);
}
