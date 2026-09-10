import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ToastProvider } from '@/components/ui/toast-provider';
import { CreateNoteDialog } from './create-note-dialog';

const renderWithProviders = (ui: React.ReactElement) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <ToastProvider>{ui}</ToastProvider>
    </QueryClientProvider>,
  );
};

vi.mock('./note-editor', () => ({
  NoteEditor: ({ onSave, onCancel }: { onSave: () => void; onCancel: () => void }) => (
    <div data-testid="note-editor">
      <button data-testid="editor-save" onClick={onSave}>Save</button>
      <button data-testid="editor-cancel" onClick={onCancel}>Cancel</button>
    </div>
  ),
}));

describe('CreateNoteDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders trigger button', () => {
    renderWithProviders(
      <CreateNoteDialog>
        <button>Open Dialog</button>
      </CreateNoteDialog>,
    );
    expect(screen.getByText('Open Dialog')).toBeInTheDocument();
  });

  it('opens dialog when trigger clicked', () => {
    renderWithProviders(
      <CreateNoteDialog>
        <button>Open Dialog</button>
      </CreateNoteDialog>,
    );
    fireEvent.click(screen.getByText('Open Dialog'));
    expect(screen.getByText('Create Note')).toBeInTheDocument();
  });

  it('renders NoteEditor inside dialog', () => {
    renderWithProviders(
      <CreateNoteDialog>
        <button>Open Dialog</button>
      </CreateNoteDialog>,
    );
    fireEvent.click(screen.getByText('Open Dialog'));
    expect(screen.getByTestId('note-editor')).toBeInTheDocument();
  });

  it('closes dialog when editor saves', () => {
    renderWithProviders(
      <CreateNoteDialog>
        <button>Open Dialog</button>
      </CreateNoteDialog>,
    );
    fireEvent.click(screen.getByText('Open Dialog'));
    expect(screen.getByText('Create Note')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('editor-save'));
    expect(screen.queryByText('Create Note')).not.toBeInTheDocument();
  });

  it('closes dialog when editor cancels', () => {
    renderWithProviders(
      <CreateNoteDialog>
        <button>Open Dialog</button>
      </CreateNoteDialog>,
    );
    fireEvent.click(screen.getByText('Open Dialog'));
    expect(screen.getByText('Create Note')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('editor-cancel'));
    expect(screen.queryByText('Create Note')).not.toBeInTheDocument();
  });

  it('can be controlled externally via open/onOpenChange props', () => {
    const onOpenChange = vi.fn();
    renderWithProviders(
      <CreateNoteDialog open={true} onOpenChange={onOpenChange}>
        <button>Trigger</button>
      </CreateNoteDialog>,
    );
    expect(screen.getByText('Create Note')).toBeInTheDocument();
  });
});
