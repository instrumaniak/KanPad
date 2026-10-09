import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import { GuestRoute } from './guest-route';

let mockAuthState: {
  user: { id: number; email: string; role: string } | null;
  isLoading: boolean;
} = { user: null, isLoading: false };

vi.mock('./use-auth', () => ({
  useAuth: () => mockAuthState,
}));

function renderGuestRoute(initialEntries = ['/login']) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <Routes>
        <Route element={<GuestRoute />}>
          <Route path="/login" element={<div>Login Page</div>} />
          <Route path="/register" element={<div>Register Page</div>} />
          <Route path="/forgot-password" element={<div>Forgot Password Page</div>} />
        </Route>
        <Route path="/" element={<div>Home Page</div>} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('GuestRoute', () => {
  beforeEach(() => {
    mockAuthState = { user: null, isLoading: false };
  });

  it('renders outlet content for guests', () => {
    renderGuestRoute();

    expect(screen.getByText('Login Page')).toBeInTheDocument();
  });

  it('redirects authenticated users to home', () => {
    mockAuthState = {
      user: { id: 1, email: 'test@example.com', role: 'user' },
      isLoading: false,
    };
    renderGuestRoute();

    expect(screen.getByText('Home Page')).toBeInTheDocument();
    expect(screen.queryByText('Login Page')).not.toBeInTheDocument();
  });

  it('shows loading state instead of redirecting while auth resolves', () => {
    mockAuthState = {
      user: { id: 1, email: 'test@example.com', role: 'user' },
      isLoading: true,
    };
    renderGuestRoute();

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.queryByText('Login Page')).not.toBeInTheDocument();
  });

  it('shows loading state when user is null and auth is resolving', () => {
    mockAuthState = { user: null, isLoading: true };
    renderGuestRoute();

    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.queryByText('Login Page')).not.toBeInTheDocument();
    expect(screen.queryByText('Home Page')).not.toBeInTheDocument();
  });

  it.each([['/register', 'Register Page'], ['/forgot-password', 'Forgot Password Page']] as const)(
    'renders %s outlet content for guests',
    (path, text) => {
      renderGuestRoute([path]);

      expect(screen.getByText(text)).toBeInTheDocument();
    },
  );

  it.each([['/login'], ['/register'], ['/forgot-password']] as const)(
    'redirects authenticated users to home from %s',
    (path) => {
      mockAuthState = {
        user: { id: 1, email: 'test@example.com', role: 'user' },
        isLoading: false,
      };
      renderGuestRoute([path]);

      expect(screen.getByText('Home Page')).toBeInTheDocument();
    },
  );
});
