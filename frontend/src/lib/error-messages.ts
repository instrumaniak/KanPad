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

export function getErrorMessage(error: unknown): ErrorMessage {
  if (error instanceof TypeError && error.message.includes('Failed to fetch')) {
    return ERROR_MESSAGES.network;
  }
  if (error instanceof Error && error.name === 'AbortError') {
    return ERROR_MESSAGES.timeout;
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const status = (error as any)?.response?.status ?? (error as any)?.status;
  if (typeof status === 'number') {
    if (status === 401) return ERROR_MESSAGES.unauthorized;
    if (status >= 500) return ERROR_MESSAGES.server;
    if (status >= 400) return ERROR_MESSAGES.validation;
  }
  return ERROR_MESSAGES.unknown;
}
