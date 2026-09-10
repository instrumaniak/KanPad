import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { LoginForm } from './login-form';
import { AuthProvider } from './auth-provider';
import { ToastProvider } from '@/components/ui/toast-provider';

vi.mock('./auth.api', () => ({
  registerApi: vi.fn(),
  loginApi: vi.fn(),
  meApi: vi.fn().mockRejectedValue(new Error('Not authenticated')),
  logoutApi: vi.fn(),
}));

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <ToastProvider>
          <AuthProvider>{children}</AuthProvider>
        </ToastProvider>
      </BrowserRouter>
    </QueryClientProvider>
  );
};

describe('LoginForm', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders form with email and password fields', () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    expect(screen.getByLabelText(/email/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/^password$/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /sign in/i })).toBeInTheDocument();
  });

  it('renders the form with correct structure', () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    expect(screen.getByText(/KanPad/i)).toBeInTheDocument();
    expect(screen.getByText(/enter your credentials to sign in/i)).toBeInTheDocument();
  });

  it('allows form input', () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    const emailInput = screen.getByLabelText(/email/i);
    const passwordInput = screen.getByLabelText(/^password$/i);

    fireEvent.change(emailInput, { target: { value: 'test@example.com' } });
    fireEvent.change(passwordInput, { target: { value: 'Password123' } });

    expect(emailInput).toHaveValue('test@example.com');
    expect(passwordInput).toHaveValue('Password123');
  });

  it('shows validation errors for empty fields on submit', async () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      expect(screen.getByText('Email is required')).toBeInTheDocument();
      expect(screen.getByText('Password is required')).toBeInTheDocument();
    });
  });

  it('shows validation error for invalid email on submit', async () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    fireEvent.change(screen.getByLabelText(/email/i), { target: { value: 'invalid' } });
    fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'Password123' } });
    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      expect(screen.getByText('Please enter a valid email')).toBeInTheDocument();
    });
  });

  it('renders validation errors with text-destructive class', async () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      const errorElements = screen.getAllByText(/is required/i);
      errorElements.forEach((el) => {
        expect(el.className).toContain('text-destructive');
      });
    });
  });

  it('renders link to register page', () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    expect(screen.getByText(/don't have an account/i)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /register/i })).toBeInTheDocument();
  });

  it('renders password visibility toggle button', () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    expect(screen.getByRole('button', { name: /show password/i })).toBeInTheDocument();
  });

  it('toggles password input type when clicking the visibility toggle', () => {
    render(<LoginForm />, { wrapper: createWrapper() });

    const passwordInput = screen.getByLabelText(/^password$/i);
    const toggleButton = screen.getByRole('button', { name: /show password/i });

    expect(passwordInput).toHaveAttribute('type', 'password');

    fireEvent.click(toggleButton);

    expect(passwordInput).toHaveAttribute('type', 'text');
    expect(screen.getByRole('button', { name: /hide password/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /hide password/i }));

    expect(passwordInput).toHaveAttribute('type', 'password');
    expect(screen.getByRole('button', { name: /show password/i })).toBeInTheDocument();
  });
});
