import type { Preview } from '@storybook/react-vite';
import React, { useLayoutEffect } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { ToastProvider } from '../src/components/ui/toast-provider';
import { AuthContext } from '../src/features/auth/auth-context';
import type { AuthContextType, User } from '../src/features/auth/auth-context';
import '../src/index.css';
import { setupWorker } from 'msw/browser';
import { mswLoader } from 'msw-storybook-addon/csf3';

const fakeUser: User = { id: 1, email: 'demo@example.com', role: 'user' };

const authValue: AuthContextType = {
  user: fakeUser,
  isLoading: false,
  register: async () => {},
  login: async () => {},
  logout: async () => {},
};

function ThemedProviders({ theme, children }: { theme: string; children?: React.ReactNode }) {
  useLayoutEffect(() => {
    const root = document.documentElement;
    const prevClass = root.classList.contains('dark')
      ? 'dark'
      : root.classList.contains('light')
        ? 'light'
        : null;
    const prevStorage = (() => {
      try {
        return localStorage.getItem('theme');
      } catch {
        return null;
      }
    })();
    root.classList.remove('light', 'dark');
    root.classList.add(theme);
    try {
      localStorage.setItem('theme', theme);
    } catch {
      /* ignore */
    }
    return () => {
      root.classList.remove('light', 'dark');
      if (prevClass) root.classList.add(prevClass);
      try {
        if (prevStorage === null) localStorage.removeItem('theme');
        else localStorage.setItem('theme', prevStorage);
      } catch {
        /* ignore */
      }
    };
  }, [theme]);

  const client = React.useMemo(
    () =>
      new QueryClient({
        defaultOptions: { queries: { staleTime: 5 * 60 * 1000, retry: false } },
      }),
    [],
  );

  return React.createElement(
    QueryClientProvider,
    { client },
    React.createElement(
      MemoryRouter,
      null,
      React.createElement(
        ToastProvider,
        null,
        React.createElement(AuthContext.Provider, { value: authValue }, children),
      ),
    ),
  );
}

const preview: Preview = {
  initialGlobals: {
    theme: 'light',
  },
  globalTypes: {
    theme: {
      name: 'Theme',
      description: 'Global theme for components',
      toolbar: {
        icon: 'circlehollow',
        items: [
          { value: 'light', icon: 'sun', title: 'Light' },
          { value: 'dark', icon: 'moon', title: 'Dark' },
        ],
        // NB: Storybook 10 has no `showName` toolbar option (it was dropped);
        // the button shows the item icon + tooltip.
      },
    },
  },
  decorators: [
    (Story, context) => {
      const theme = (context.globals.theme as string) || 'light';
      return React.createElement(
        ThemedProviders,
        { theme },
        React.createElement(Story, null),
      );
    },
  ],
  loaders: [
    mswLoader(async () => {
      const worker = setupWorker();
      await worker.start({
        onUnhandledFrame({ frame, defaults }) {
          if (frame.protocol === 'http') {
            const request = (frame as { data?: { request?: Request } }).data?.request;
            if (request && new URL(request.url).pathname.startsWith('/api')) {
              defaults.error();
            }
          }
        },
      });
      return worker;
    }),
  ],
  parameters: {
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
    a11y: {
      test: 'error',
    },
  },
  tags: ['autodocs'],
};

export default preview;
