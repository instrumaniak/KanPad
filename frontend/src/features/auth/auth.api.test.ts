import { describe, it, expect, vi, beforeEach } from 'vitest';
import { apiFetch, apiFetchVoid, BASE } from '@/lib/api/client';
import type { components } from '@/lib/api/schema';
import { registerApi, loginApi, meApi, logoutApi } from './auth.api';

vi.mock('@/lib/api/client', () => ({
  apiFetch: vi.fn(),
  apiFetchVoid: vi.fn(),
  BASE: '',
  ApiError: class ApiError extends Error {
    status: number;
    constructor(status: number, message: string) {
      super(message);
      this.status = status;
    }
  },
}));

type UserData = components['schemas']['UserData'];
type RegisterResponse = components['schemas']['RegisterResponse'];
type LoginResponse = components['schemas']['LoginResponse'];
type MeResponse = components['schemas']['MeResponse'];

const mockApiFetch = apiFetch as ReturnType<typeof vi.fn>;
const mockApiFetchVoid = apiFetchVoid as ReturnType<typeof vi.fn>;

const userFixture: UserData = { id: 1, email: 'test@example.com', role: 'user' };

describe('auth.api', () => {
  beforeEach(() => vi.clearAllMocks());

  describe('registerApi', () => {
    it('registers user with valid data', async () => {
      const mockResponse: RegisterResponse = { data: userFixture };
      mockApiFetch.mockResolvedValueOnce(mockResponse);

      const result = await registerApi({ email: 'test@example.com', password: 'Test1234!' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/auth/register`, {
        method: 'POST',
        body: JSON.stringify({ email: 'test@example.com', password: 'Test1234!' }),
      });
      expect(result).toEqual(mockResponse);
    });

    it('throws error on duplicate email', async () => {
      mockApiFetch.mockRejectedValueOnce(Object.assign(new Error('Email already registered'), { status: 409 }));

      await expect(registerApi({ email: 'exists@example.com', password: 'Test1234!' })).rejects.toThrow(
        'Email already registered',
      );
    });
  });

  describe('loginApi', () => {
    it('logs in with valid credentials', async () => {
      const mockResponse: LoginResponse = { data: userFixture };
      mockApiFetch.mockResolvedValueOnce(mockResponse);

      const result = await loginApi({ email: 'test@example.com', password: 'Test1234!' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/auth/login`, {
        method: 'POST',
        body: JSON.stringify({ email: 'test@example.com', password: 'Test1234!' }),
      });
      expect(result).toEqual(mockResponse);
    });

    it('propagates fetch rejections unchanged', async () => {
      mockApiFetch.mockRejectedValueOnce(new TypeError('Failed to fetch'));

      await expect(loginApi({ email: 'test@example.com', password: 'Test1234!' })).rejects.toThrow(
        'Failed to fetch',
      );
    });

    it('throws error for invalid credentials', async () => {
      mockApiFetch.mockRejectedValueOnce(
        Object.assign(new Error('Invalid email or password'), { status: 401 }),
      );

      await expect(loginApi({ email: 'test@example.com', password: 'WrongPass' })).rejects.toThrow(
        'Invalid email or password',
      );
    });
  });

  describe('meApi', () => {
    it('returns current user when authenticated', async () => {
      const mockResponse: MeResponse = { data: userFixture };
      mockApiFetch.mockResolvedValueOnce(mockResponse);

      const result = await meApi();
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/auth/me`);
      expect(result).toEqual(mockResponse);
    });

    it('throws error when not authenticated', async () => {
      mockApiFetch.mockRejectedValueOnce(Object.assign(new Error('Unauthorized'), { status: 401 }));

      await expect(meApi()).rejects.toThrow('Unauthorized');
    });
  });

  describe('logoutApi', () => {
    it('logs out successfully via the void helper', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      await expect(logoutApi()).resolves.toBeUndefined();
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/auth/logout`, { method: 'POST' });
      expect(mockApiFetch).not.toHaveBeenCalled();
    });

    it('throws error on logout failure', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(Object.assign(new Error('Server error'), { status: 500 }));

      await expect(logoutApi()).rejects.toThrow('Server error');
    });
  });
});
