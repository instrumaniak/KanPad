import { useState, useEffect } from 'react';

type Breakpoint = 'mobile' | 'tablet' | 'desktop';

const BREAKPOINTS = {
  mobile: { max: '40rem' },
  tablet: { min: '40rem', max: '63.9375rem' },
  desktop: { min: '64rem' },
} as const;

function getBreakpoint(width: number): Breakpoint {
  if (width < 640) return 'mobile';
  if (width < 1024) return 'tablet';
  return 'desktop';
}

export function useBreakpoint(): Breakpoint {
  const [breakpoint, setBreakpoint] = useState<Breakpoint>(() => {
    if (typeof window === 'undefined') return 'desktop';
    return getBreakpoint(window.innerWidth);
  });

  useEffect(() => {
    // Both boundaries must be observed: a tablet-to-desktop resize does not
    // change a query that only watches the mobile boundary.
    const mediaQueries = [
      window.matchMedia('(max-width: 39.9375rem)'),
      window.matchMedia(`(min-width: ${BREAKPOINTS.desktop.min})`),
    ];

    const handleChange = () => {
      setBreakpoint(getBreakpoint(window.innerWidth));
    };

    mediaQueries.forEach((mediaQuery) => mediaQuery.addEventListener('change', handleChange));

    return () => {
      mediaQueries.forEach((mediaQuery) => mediaQuery.removeEventListener('change', handleChange));
    };
  }, []);

  return breakpoint;
}
