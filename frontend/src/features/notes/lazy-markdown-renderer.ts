import { lazy } from 'react';

export const LazyMarkdownRenderer = lazy(() =>
  import('./markdown-renderer').then((m) => ({ default: m.MarkdownRenderer })),
);
