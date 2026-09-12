import { describe, it, expect } from 'vitest';
import { getErrorMessage, ERROR_MESSAGES } from './error-messages';

describe('getErrorMessage', () => {
  it('returns network error for TypeError with Failed to fetch', () => {
    const error = new TypeError('Failed to fetch');
    expect(getErrorMessage(error)).toEqual(ERROR_MESSAGES.network);
  });

  it('returns timeout error for AbortError', () => {
    const error = new Error('Aborted');
    error.name = 'AbortError';
    expect(getErrorMessage(error)).toEqual(ERROR_MESSAGES.timeout);
  });

  it('returns unauthorized error for 401 Response', () => {
    const response = new Response(null, { status: 401 });
    expect(getErrorMessage(response)).toEqual(ERROR_MESSAGES.unauthorized);
  });

  it('returns server error for 500 Response', () => {
    const response = new Response(null, { status: 500 });
    expect(getErrorMessage(response)).toEqual(ERROR_MESSAGES.server);
  });

  it('returns validation error for 400 Response', () => {
    const response = new Response(null, { status: 400 });
    expect(getErrorMessage(response)).toEqual(ERROR_MESSAGES.validation);
  });

  it('returns unauthorized error for error with status 401', () => {
    const error = { response: { status: 401 } };
    expect(getErrorMessage(error)).toEqual(ERROR_MESSAGES.unauthorized);
  });

  it('returns server error for error with status 500', () => {
    const error = { response: { status: 500 } };
    expect(getErrorMessage(error)).toEqual(ERROR_MESSAGES.server);
  });

  it('returns validation error for error with status 400', () => {
    const error = { response: { status: 400 } };
    expect(getErrorMessage(error)).toEqual(ERROR_MESSAGES.validation);
  });

  it('returns unknown error for unrecognized error', () => {
    expect(getErrorMessage('something')).toEqual(ERROR_MESSAGES.unknown);
  });

  it('returns unknown error for null', () => {
    expect(getErrorMessage(null)).toEqual(ERROR_MESSAGES.unknown);
  });
});
