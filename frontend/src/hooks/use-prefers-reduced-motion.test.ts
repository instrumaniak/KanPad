import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { usePrefersReducedMotion } from './use-prefers-reduced-motion';

function mockMatchMedia(reducedMotion: boolean) {
  const listeners: Array<(e: MediaQueryListEvent) => void> = [];
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: reducedMotion,
      media: query,
      onchange: null,
      addEventListener: (_: string, handler: (e: MediaQueryListEvent) => void) => {
        listeners.push(handler);
      },
      removeEventListener: (_: string, handler: (e: MediaQueryListEvent) => void) => {
        const idx = listeners.indexOf(handler);
        if (idx >= 0) listeners.splice(idx, 1);
      },
      dispatchEvent: vi.fn(),
    })),
  });
  return listeners;
}

describe('usePrefersReducedMotion', () => {
  beforeEach(() => {
    mockMatchMedia(false);
  });

  it('returns false when prefers-reduced-motion is no-preference', () => {
    mockMatchMedia(false);
    const { result } = renderHook(() => usePrefersReducedMotion());
    expect(result.current).toBe(false);
  });

  it('returns true when prefers-reduced-motion is reduce', () => {
    mockMatchMedia(true);
    const { result } = renderHook(() => usePrefersReducedMotion());
    expect(result.current).toBe(true);
  });

  it('updates when media query changes', () => {
    const listeners = mockMatchMedia(false);
    const { result } = renderHook(() => usePrefersReducedMotion());
    expect(result.current).toBe(false);

    act(() => {
      listeners.forEach((handler) =>
        handler({ matches: true } as MediaQueryListEvent),
      );
    });
    expect(result.current).toBe(true);
  });
});
