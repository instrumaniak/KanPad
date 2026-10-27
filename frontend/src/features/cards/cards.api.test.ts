import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { components } from '@/lib/api/schema';
import { BASE, ApiError } from '@/lib/api/client';
import {
  createCard,
  updateCard,
  deleteCard,
  fetchCards,
  fetchCard,
  assignLabelToCard,
  removeLabelFromCard,
  sanitizeCardListParams,
  CARD_LIST_DEFAULT_PAGE,
  CARD_LIST_DEFAULT_LIMIT,
} from './cards.api';

vi.mock('@/lib/api/client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/lib/api/client')>();
  return {
    ...actual,
    apiFetch: vi.fn(),
    apiFetchVoid: vi.fn(),
  };
});

import { apiFetch, apiFetchVoid } from '@/lib/api/client';

const mockApiFetch = apiFetch as ReturnType<typeof vi.fn>;
const mockApiFetchVoid = apiFetchVoid as ReturnType<typeof vi.fn>;

type CardSummary = components['schemas']['CardSummary'];
type CardDetail = components['schemas']['CardDetailResponse'];

function summaryFixture(overrides?: Partial<CardSummary>): CardSummary {
  return {
    id: 1,
    title: 'Card 1',
    column_id: 1,
    position: 0,
    has_description: false,
    due_date: null,
    labels: [],
    created_at: '2024-01-01T00:00:00.000000Z',
    updated_at: '2024-01-01T00:00:00.000000Z',
    ...overrides,
  };
}

function detailFixture(overrides?: Partial<CardDetail>): CardDetail {
  return {
    id: 1,
    title: 'Card 1',
    column_id: 1,
    position: 0,
    description: 'A detail',
    due_date: null,
    labels: [],
    checklists: [],
    created_at: '2024-01-01T00:00:00.000000Z',
    updated_at: '2024-01-01T00:00:00.000000Z',
    ...overrides,
  };
}

describe('cards.api', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchCards', () => {
    it('fetches a page of card summaries for a column', async () => {
      const body: components['schemas']['Page_CardSummary'] = {
        data: [summaryFixture(), summaryFixture({ id: 2, title: 'Card 2', position: 1 })],
        page: 1,
        limit: 50,
        total: 2,
      };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await fetchCards(1);
      expect(mockApiFetch).toHaveBeenCalledWith(
        `${BASE}/api/v1/columns/1/cards?page=${CARD_LIST_DEFAULT_PAGE}&limit=${CARD_LIST_DEFAULT_LIMIT}`,
      );
      expect(result).toEqual(body);
    });

    it('passes explicit page/limit through', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 2, limit: 10, total: 0 });

      await fetchCards(1, 2, 10);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/columns/1/cards?page=2&limit=10`);
    });

    it('propagates ApiError on failed response', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Column not found'));
      await expect(fetchCards(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('sanitizeCardListParams', () => {
    it('falls back to defaults on garbage (newly-strict 400 audit)', async () => {
      expect(sanitizeCardListParams()).toEqual({
        page: CARD_LIST_DEFAULT_PAGE,
        limit: CARD_LIST_DEFAULT_LIMIT,
      });
      expect(sanitizeCardListParams(0, 500)).toEqual({
        page: CARD_LIST_DEFAULT_PAGE,
        limit: CARD_LIST_DEFAULT_LIMIT,
      });
      expect(sanitizeCardListParams(Number.NaN, -1)).toEqual({
        page: CARD_LIST_DEFAULT_PAGE,
        limit: CARD_LIST_DEFAULT_LIMIT,
      });
      expect(sanitizeCardListParams(2.5, 10)).toEqual({ page: 1, limit: 10 });
    });

    it('never emits garbage query params', async () => {
      mockApiFetch.mockResolvedValueOnce({ data: [], page: 1, limit: 50, total: 0 });

      await fetchCards(1, Number.NaN, 500);
      expect(mockApiFetch).toHaveBeenCalledWith(
        `${BASE}/api/v1/columns/1/cards?page=1&limit=50`,
      );
    });
  });

  describe('fetchCard', () => {
    it('fetches a single card detail by id', async () => {
      const body: components['schemas']['CardBody'] = { data: detailFixture() };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await fetchCard(1);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/cards/1`);
      expect(result).toEqual(body);
    });

    it('propagates ApiError on failed fetch', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Card not found'));
      await expect(fetchCard(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('createCard', () => {
    it('creates a new card', async () => {
      const body: components['schemas']['CardBody'] = {
        data: detailFixture({ title: 'New Card', description: null }),
      };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await createCard({ title: 'New Card', column_id: 1 });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/cards`, {
        method: 'POST',
        body: JSON.stringify({ title: 'New Card', column_id: 1 }),
      });
      expect(result).toEqual(body);
    });

    it('propagates ApiError on failed create', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Column not found'));
      await expect(createCard({ title: 'Test', column_id: 999 })).rejects.toMatchObject({
        status: 404,
      });
    });
  });

  describe('updateCard', () => {
    it('updates a card title', async () => {
      const body: components['schemas']['CardBody'] = {
        data: detailFixture({ title: 'Updated Card' }),
      };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await updateCard(1, { title: 'Updated Card' });
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/cards/1`, {
        method: 'PATCH',
        body: JSON.stringify({ title: 'Updated Card' }),
      });
      expect(result).toEqual(body);
    });

    it('moves a card across columns', async () => {
      const body: components['schemas']['CardBody'] = {
        data: detailFixture({ column_id: 2, position: 5 }),
      };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await updateCard(1, { column_id: 2, position: 5 });
      expect(mockApiFetch).toHaveBeenCalledWith(
        `${BASE}/api/v1/cards/1`,
        expect.objectContaining({
          method: 'PATCH',
          body: JSON.stringify({ column_id: 2, position: 5 }),
        }),
      );
      expect(result).toEqual(body);
    });

    it('propagates ApiError on failed update', async () => {
      mockApiFetch.mockRejectedValueOnce(new ApiError(404, 'Card not found'));
      await expect(updateCard(999, { title: 'Test' })).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('deleteCard', () => {
    it('deletes a card via the void transport (204)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await deleteCard(1);
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/cards/1`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });

    it('propagates ApiError on failed delete', async () => {
      mockApiFetchVoid.mockRejectedValueOnce(new ApiError(404, 'Card not found'));
      await expect(deleteCard(999)).rejects.toMatchObject({ status: 404 });
    });
  });

  describe('assignLabelToCard', () => {
    it('attaches via canonical PUT with path ids (no body)', async () => {
      const body: components['schemas']['CardBody'] = {
        data: detailFixture({ labels: [{ id: 2, name: 'Urgent', color: 'red' }] }),
      };
      mockApiFetch.mockResolvedValueOnce(body);

      const result = await assignLabelToCard(1, 2);
      expect(mockApiFetch).toHaveBeenCalledWith(`${BASE}/api/v1/cards/1/labels/2`, {
        method: 'PUT',
      });
      expect(result).toEqual(body);
    });
  });

  describe('removeLabelFromCard', () => {
    it('detaches via the void transport (204)', async () => {
      mockApiFetchVoid.mockResolvedValueOnce(undefined);

      const result = await removeLabelFromCard(1, 2);
      expect(mockApiFetchVoid).toHaveBeenCalledWith(`${BASE}/api/v1/cards/1/labels/2`, {
        method: 'DELETE',
      });
      expect(result).toBeUndefined();
    });
  });
});
