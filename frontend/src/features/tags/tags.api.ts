import type { ApiResponse } from '../cards/cards.api';

const FETCH_OPTIONS: RequestInit = {
  credentials: 'include',
};

export interface Tag {
  id: number;
  name: string;
  color: string;
  user_id: number;
  created_at: string;
}

export interface CreateTagData {
  name: string;
  color: string;
}

export interface UpdateTagData {
  name?: string;
  color?: string;
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let message = 'Request failed';
    try {
      const error = await response.json();
      message = Array.isArray(error.message) ? error.message.join(', ') : error.message;
    } catch {
      message = response.statusText || 'Request failed';
    }
    throw new Error(message);
  }
  try {
    return await response.json();
  } catch {
    throw new Error('Unexpected response format');
  }
}

export async function fetchTags(): Promise<ApiResponse<Tag[]>> {
  let response: Response;
  try {
    response = await fetch('/api/tags', FETCH_OPTIONS);
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<Tag[]>>(response);
}

export async function createTag(data: CreateTagData): Promise<ApiResponse<Tag>> {
  let response: Response;
  try {
    response = await fetch('/api/tags', {
      ...FETCH_OPTIONS,
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<Tag>>(response);
}

export async function updateTag(id: number, data: UpdateTagData): Promise<ApiResponse<Tag>> {
  let response: Response;
  try {
    response = await fetch(`/api/tags/${id}`, {
      ...FETCH_OPTIONS,
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<Tag>>(response);
}

export async function deleteTag(id: number): Promise<ApiResponse<void>> {
  let response: Response;
  try {
    response = await fetch(`/api/tags/${id}`, {
      ...FETCH_OPTIONS,
      method: 'DELETE',
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<void>>(response);
}
