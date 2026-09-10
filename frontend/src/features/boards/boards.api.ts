interface Board {
  id: number;
  name: string;
  background_color?: string;
  project_id: number | null;
  project?: { id: number; name: string } | null;
  created_at: string;
  updated_at: string;
  columns?: { id: number; name: string; position: number }[];
  is_archived?: boolean;
  view_mode?: 'board' | 'list';
}

interface ApiResponse<T> {
  data: T;
  message?: string;
}

interface ListResponse<T> {
  data: T[];
  total: number;
}

interface ApiError {
  statusCode: number;
  message: string | string[];
  error: string;
}

const FETCH_OPTIONS: RequestInit = { credentials: 'include' };

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let message = 'Request failed';
    try {
      const error: ApiError = await response.json();
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

export async function fetchBoards(projectId?: number): Promise<ListResponse<Board>> {
  let response: Response;
  const url = projectId ? `/api/boards?projectId=${projectId}` : '/api/boards';
  try {
    response = await fetch(url, FETCH_OPTIONS);
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export async function fetchBoard(id: number): Promise<ApiResponse<Board>> {
  let response: Response;
  try {
    response = await fetch(`/api/boards/${id}`, FETCH_OPTIONS);
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export interface CreateBoardData {
  name: string;
  background_color?: string;
  project_id?: number | null;
}

export async function createBoard(data: CreateBoardData): Promise<ApiResponse<Board>> {
  let response: Response;
  try {
    response = await fetch('/api/boards', {
      ...FETCH_OPTIONS,
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export interface UpdateBoardData {
  name?: string;
  background_color?: string;
  project_id?: number | null;
  view_mode?: 'board' | 'list';
}

export async function updateBoard(id: number, data: UpdateBoardData): Promise<ApiResponse<Board>> {
  let response: Response;
  try {
    response = await fetch(`/api/boards/${id}`, {
      ...FETCH_OPTIONS,
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export async function deleteBoard(id: number): Promise<ApiResponse<void>> {
  let response: Response;
  try {
    response = await fetch(`/api/boards/${id}`, {
      ...FETCH_OPTIONS,
      method: 'DELETE',
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export async function archiveBoard(id: number): Promise<ApiResponse<Board>> {
  let response: Response;
  try {
    response = await fetch(`/api/boards/${id}/archive`, {
      ...FETCH_OPTIONS,
      method: 'PATCH',
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export async function restoreBoard(id: number): Promise<ApiResponse<Board>> {
  let response: Response;
  try {
    response = await fetch(`/api/boards/${id}/restore`, {
      ...FETCH_OPTIONS,
      method: 'PATCH',
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export async function permanentDeleteBoard(id: number): Promise<ApiResponse<void>> {
  let response: Response;
  try {
    response = await fetch(`/api/boards/${id}/permanent`, {
      ...FETCH_OPTIONS,
      method: 'DELETE',
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export async function fetchArchivedBoards(): Promise<ListResponse<Board>> {
  let response: Response;
  try {
    response = await fetch('/api/boards/archived', FETCH_OPTIONS);
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse(response);
}

export async function recreateBoard(name: string): Promise<ApiResponse<Board>> {
  return createBoard({ name });
}

export type { Board, ApiResponse, ListResponse };