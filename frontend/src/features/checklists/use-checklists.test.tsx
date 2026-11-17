import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactNode } from 'react';
import { useUpdateChecklistItem } from './use-checklists';
import { resetSessionCaches } from '@/lib/query-client-reset';

vi.mock('./checklists.api', () => ({
  createChecklist: vi.fn(),
  updateChecklist: vi.fn(),
  deleteChecklist: vi.fn(),
  createChecklistItem: vi.fn(),
  updateChecklistItem: vi.fn(),
  deleteChecklistItem: vi.fn(),
}));

import { updateChecklistItem } from './checklists.api';
const mockUpdateChecklistItem = updateChecklistItem as ReturnType<typeof vi.fn>;

const cardFixture = () => ({
  id: 7,
  title: 'Card with checklist',
  checklists: [
    {
      id: 11,
      title: 'Definition of done',
      items: [{ id: 9, is_completed: false, position: 0, text: 'Ship it' }],
    },
  ],
});

const createClient = () =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });

describe('useUpdateChecklistItem post-boundary guard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('does NOT recreate [card, id] after a session boundary mid-flight', async () => {
    const queryClient = createClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );

    queryClient.setQueryData(['card', 7], cardFixture());

    let rejectUpdate!: (err: unknown) => void;
    mockUpdateChecklistItem.mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectUpdate = reject;
      }),
    );

    const { result } = renderHook(() => useUpdateChecklistItem(), { wrapper });

    result.current.mutate({ itemId: 9, data: { is_completed: true }, cardId: 7 });

    // Optimistic toggle applied.
    await waitFor(() => {
      const cached = queryClient.getQueryData<{
        checklists: { items: { id: number; is_completed: boolean }[] }[];
      }>(['card', 7]);
      expect(cached?.checklists[0]?.items[0]?.is_completed).toBe(true);
    });

    // Session boundary lands while the mutation is in flight.
    await resetSessionCaches(queryClient);

    rejectUpdate(new Error('Update failed'));

    await waitFor(() => expect(result.current.isError).toBe(true));

    // The rollback MUST NOT recreate ['card', 7] with the previous user's
    // snapshot.
    expect(queryClient.getQueryData(['card', 7])).toBeUndefined();
  });

  it('rolls back the optimistic toggle as usual when NO boundary intervenes', async () => {
    const queryClient = createClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );

    queryClient.setQueryData(['card', 7], cardFixture());
    mockUpdateChecklistItem.mockRejectedValue(new Error('Update failed'));

    const { result } = renderHook(() => useUpdateChecklistItem(), { wrapper });

    result.current.mutate({ itemId: 9, data: { is_completed: true }, cardId: 7 });

    await waitFor(() => expect(result.current.isError).toBe(true));

    // Same epoch — the previous snapshot MUST be restored (guard must not
    // over-fence: rollback only dies at a session boundary).
    const cached = queryClient.getQueryData<{
      checklists: { items: { id: number; is_completed: boolean }[] }[];
    }>(['card', 7]);
    expect(cached?.checklists[0]?.items[0]?.is_completed).toBe(false);
  });
});
