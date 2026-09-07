import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RegisterForm } from './register-form';
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

describe('RegisterForm', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders form fields', () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    expect(screen.getByLabelText(/email/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/^password$/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/confirm password/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /register/i })).toBeInTheDocument();
  });

  it('renders the form with correct structure', () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    expect(screen.getByText(/KanPad/i)).toBeInTheDocument();
    expect(screen.getByText(/enter your details to register/i)).toBeInTheDocument();
  });

  it('allows form input', () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    const emailInput = screen.getByLabelText(/email/i);
    const passwordInput = screen.getByLabelText(/^password$/i);
    const confirmInput = screen.getByLabelText(/confirm password/i);

    fireEvent.change(emailInput, { target: { value: 'test@example.com' } });
    fireEvent.change(passwordInput, { target: { value: 'Password123' } });
    fireEvent.change(confirmInput, { target: { value: 'Password123' } });

    expect(emailInput).toHaveValue('test@example.com');
    expect(passwordInput).toHaveValue('Password123');
    expect(confirmInput).toHaveValue('Password123');
  });

  it('shows validation errors for empty fields on submit', async () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      expect(screen.getByText('Email is required')).toBeInTheDocument();
      expect(screen.getByText('Password is required')).toBeInTheDocument();
      expect(screen.getByText('Please confirm your password')).toBeInTheDocument();
    });
  });

  it('shows validation error for invalid email on submit', async () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    fireEvent.change(screen.getByLabelText(/email/i), { target: { value: 'invalid' } });
    fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'Password123' } });
    fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'Password123' } });
    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      expect(screen.getByText('Please enter a valid email')).toBeInTheDocument();
    });
  });

  it('shows validation error for weak password on submit', async () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    fireEvent.change(screen.getByLabelText(/email/i), { target: { value: 'test@example.com' } });
    fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'short' } });
    fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'short' } });
    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      expect(screen.getByText('Password must be at least 8 characters')).toBeInTheDocument();
    });
  });

  it('shows validation error for password without number or special char', async () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    fireEvent.change(screen.getByLabelText(/email/i), { target: { value: 'test@example.com' } });
    fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'longpassword' } });
    fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'longpassword' } });
    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      expect(screen.getByText('Password must contain at least 1 number or special character')).toBeInTheDocument();
    });
  });

  it('shows validation error for mismatched passwords on submit', async () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    fireEvent.change(screen.getByLabelText(/email/i), { target: { value: 'test@example.com' } });
    fireEvent.change(screen.getByLabelText(/^password$/i), { target: { value: 'Password123' } });
    fireEvent.change(screen.getByLabelText(/confirm password/i), { target: { value: 'Different456' } });
    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      expect(screen.getByText('Passwords do not match')).toBeInTheDocument();
    });
  });

  it('renders validation errors with text-destructive class', async () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    fireEvent.submit(screen.getByRole('form'));

    await waitFor(() => {
      const errorElements = screen.getAllByText(/is required/i);
      errorElements.forEach((el) => {
        expect(el.className).toContain('text-destructive');
      });
    });
  });

  it('renders password visibility toggle buttons for both password fields', () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    const toggles = screen.getAllByRole('button', { name: /show password/i });
    expect(toggles).toHaveLength(2);
  });

  it('toggles password field visibility independently', () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    const passwordInput = screen.getByLabelText(/^password$/i);
    const confirmInput = screen.getByLabelText(/confirm password/i);
    const toggles = screen.getAllByRole('button', { name: /show password/i });

    expect(passwordInput).toHaveAttribute('type', 'password');
    expect(confirmInput).toHaveAttribute('type', 'password');

    fireEvent.click(toggles[0]);

    expect(passwordInput).toHaveAttribute('type', 'text');
    expect(confirmInput).toHaveAttribute('type', 'password');

    fireEvent.click(toggles[1]);

    expect(passwordInput).toHaveAttribute('type', 'text');
    expect(confirmInput).toHaveAttribute('type', 'text');
  });

  it('toggles confirm password visibility on button click', () => {
    render(<RegisterForm />, { wrapper: createWrapper() });

    const confirmInput = screen.getByLabelText(/confirm password/i);
    const toggleButtons = screen.getAllByRole('button', { name: /show password/i });

    fireEvent.click(toggleButtons[1]);

    expect(confirmInput).toHaveAttribute('type', 'text');
    expect(screen.getAllByRole('button', { name: /hide password/i })).toHaveLength(1);

    fireEvent.click(screen.getByRole('button', { name: /hide password/i }));

    expect(confirmInput).toHaveAttribute('type', 'password');
  });
});
