import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useBreakpoint } from './use-breakpoint';

function mockMatchMedia(width: number) {
  const listeners: Array<{ query: string; handler: (e: MediaQueryListEvent) => void }> = [];
  const matches = (query: string, viewportWidth: number) => {
    if (query === '(max-width: 39.9375rem)') return viewportWidth < 640;
    if (query === '(min-width: 64rem)') return viewportWidth >= 1024;
    return false;
  };

  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: matches(query, width),
      media: query,
      onchange: null,
      addEventListener: (_: string, handler: (e: MediaQueryListEvent) => void) => {
        listeners.push({ query, handler });
      },
      removeEventListener: (_: string, handler: (e: MediaQueryListEvent) => void) => {
        const idx = listeners.findIndex((listener) => listener.handler === handler);
        if (idx >= 0) listeners.splice(idx, 1);
      },
      dispatchEvent: vi.fn(),
    })),
  });

  Object.defineProperty(window, 'innerWidth', {
    writable: true,
    value: width,
  });

  return {
    listeners,
    triggerWidth(newWidth: number) {
      Object.defineProperty(window, 'innerWidth', { value: newWidth });
      listeners.forEach(({ query, handler }) => {
        if (matches(query, width) !== matches(query, newWidth)) {
          handler({ matches: matches(query, newWidth) } as MediaQueryListEvent);
        }
      });
      width = newWidth;
    },
  };
}

describe('useBreakpoint', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('returns mobile for viewport < 640px', () => {
    mockMatchMedia(360);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current).toBe('mobile');
  });

  it('returns tablet for viewport 640-1023px', () => {
    mockMatchMedia(768);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current).toBe('tablet');
  });

  it('returns desktop for viewport >= 1024px', () => {
    mockMatchMedia(1280);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current).toBe('desktop');
  });

  it('returns mobile at exactly 639px', () => {
    mockMatchMedia(639);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current).toBe('mobile');
  });

  it('returns tablet at exactly 640px', () => {
    mockMatchMedia(640);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current).toBe('tablet');
  });

  it('returns tablet at exactly 1023px', () => {
    mockMatchMedia(1023);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current).toBe('tablet');
  });

  it('returns desktop at exactly 1024px', () => {
    mockMatchMedia(1024);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current).toBe('desktop');
  });

  it('updates breakpoint on viewport change', () => {
    const { triggerWidth } = mockMatchMedia(360);
    const { result } = renderHook(() => useBreakpoint());

    expect(result.current).toBe('mobile');

    act(() => {
      triggerWidth(768);
    });
    expect(result.current).toBe('tablet');

    act(() => {
      triggerWidth(1280);
    });
    expect(result.current).toBe('desktop');

    act(() => {
      triggerWidth(500);
    });
    expect(result.current).toBe('mobile');
  });

  it('updates between tablet and desktop when the mobile query remains unmatched', () => {
    const { triggerWidth } = mockMatchMedia(800);
    const { result } = renderHook(() => useBreakpoint());

    expect(result.current).toBe('tablet');

    act(() => {
      triggerWidth(1100);
    });
    expect(result.current).toBe('desktop');

    act(() => {
      triggerWidth(800);
    });
    expect(result.current).toBe('tablet');
  });

  it('handles orientation change that crosses breakpoint', () => {
    const { triggerWidth } = mockMatchMedia(375);
    const { result } = renderHook(() => useBreakpoint());

    expect(result.current).toBe('mobile');

    act(() => {
      triggerWidth(812);
    });
    expect(result.current).toBe('tablet');

    act(() => {
      triggerWidth(375);
    });
    expect(result.current).toBe('mobile');
  });

  it('cleans up event listener on unmount', () => {
    const { triggerWidth } = mockMatchMedia(360);
    const { unmount } = renderHook(() => useBreakpoint());

    unmount();

    expect(() => {
      triggerWidth(1280);
    }).not.toThrow();
  });
});
