import type { components } from '@/lib/api/schema';
import { apiFetch, apiFetchVoid, type ApiResponse } from '@/lib/api/client';

export type { ApiResponse };
export type RegisterRequest = components['schemas']['RegisterInput'];
export type LoginRequest = components['schemas']['LoginInput'];
export type UserResponse = components['schemas']['UserData'];

export async function registerApi(data: RegisterRequest): Promise<ApiResponse<UserResponse>> {
  return apiFetch<ApiResponse<UserResponse>>('/api/auth/register', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function loginApi(data: LoginRequest): Promise<ApiResponse<UserResponse>> {
  return apiFetch<ApiResponse<UserResponse>>('/api/auth/login', {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function meApi(): Promise<ApiResponse<UserResponse>> {
  return apiFetch<ApiResponse<UserResponse>>('/api/auth/me');
}

export async function logoutApi(): Promise<void> {
  return apiFetchVoid('/api/auth/logout', { method: 'POST' });
}
