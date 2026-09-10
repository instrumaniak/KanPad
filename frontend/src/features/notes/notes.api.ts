import type { ApiResponse } from '../cards/cards.api';

const FETCH_OPTIONS: RequestInit = {
  credentials: 'include',
};

export interface Note {
  id: number;
  title: string;
  content: string;
  board_id?: number | null;
  project_id?: number | null;
  card_id?: number | null;
  user_id: number;
  tags?: Tag[];
  created_at: string;
  updated_at: string;
}

export interface Tag {
  id: number;
  name: string;
  color: string;
}

export type NoteType = 'general' | 'board' | 'project' | 'card';

export function getNoteType(note: Pick<Note, 'card_id' | 'project_id' | 'board_id'>): NoteType {
  if (note.card_id) return 'card';
  if (note.project_id) return 'project';
  if (note.board_id) return 'board';
  return 'general';
}

export interface CreateNoteData {
  title: string;
  content: string;
  board_id?: number | null;
  project_id?: number | null;
  card_id?: number | null;
  tagIds?: number[];
}

export interface UpdateNoteData {
  title?: string;
  content?: string;
  board_id?: number | null;
  project_id?: number | null;
  card_id?: number | null;
  tagIds?: number[];
}

export interface NoteFilters {
  search?: string;
  type?: NoteType;
  tagId?: number;
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

export async function createNote(data: CreateNoteData): Promise<ApiResponse<Note>> {
  let response: Response;
  try {
    response = await fetch('/api/notes', {
      ...FETCH_OPTIONS,
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<Note>>(response);
}

export async function fetchNotes(filters?: NoteFilters): Promise<ApiResponse<Note[]>> {
  let response: Response;
  const params = new URLSearchParams();
  if (filters?.search) params.set('search', filters.search);
  if (filters?.type) params.set('type', filters.type);
  if (filters?.tagId) params.set('tagId', String(filters.tagId));
  const query = params.toString();
  try {
    response = await fetch(`/api/notes${query ? `?${query}` : ''}`, FETCH_OPTIONS);
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<Note[]>>(response);
}

export async function fetchNote(id: number): Promise<ApiResponse<Note>> {
  let response: Response;
  try {
    response = await fetch(`/api/notes/${id}`, FETCH_OPTIONS);
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<Note>>(response);
}

export async function updateNote(id: number, data: UpdateNoteData): Promise<ApiResponse<Note>> {
  let response: Response;
  try {
    response = await fetch(`/api/notes/${id}`, {
      ...FETCH_OPTIONS,
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<Note>>(response);
}

export async function deleteNote(id: number): Promise<ApiResponse<void>> {
  let response: Response;
  try {
    response = await fetch(`/api/notes/${id}`, {
      ...FETCH_OPTIONS,
      method: 'DELETE',
    });
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<void>>(response);
}

export async function fetchBoardNotes(boardId: number): Promise<ApiResponse<Note[]>> {
  let response: Response;
  try {
    response = await fetch(`/api/boards/${boardId}/notes`, FETCH_OPTIONS);
  } catch {
    throw new Error('Network error — please check your connection');
  }
  return handleResponse<ApiResponse<Note[]>>(response);
}
