import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import { AppLayout } from './app-layout';

// Mock window.matchMedia for jsdom
function setupMatchMedia() {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

const mockLogout = vi.fn();
vi.mock('@/features/auth/use-auth', () => ({
  useAuth: () => ({
    user: { id: 1, email: 'test@example.com', role: 'user' },
    isLoading: false,
    logout: mockLogout,
  }),
  AuthProvider: ({ children }: { children: React.ReactNode }) => children,
}));

vi.mock('@/hooks/use-theme', () => ({
  useTheme: () => ({
    theme: 'light',
    toggleTheme: vi.fn(),
    setTheme: vi.fn(),
  }),
}));

// Mock Outlet to render child content
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual('react-router-dom');
  return {
    ...actual,
    Outlet: () => <div data-testid="outlet">Outlet Content</div>,
  };
});

vi.mock('@/features/notes', () => ({
  BoardNotesSidebar: ({ boardId }: { boardId: number }) => (
    <aside role="complementary" data-testid="board-notes-sidebar">
      Board Notes Sidebar {boardId}
    </aside>
  ),
}));

function renderWithRouter(ui: React.ReactElement, initialEntries?: string[]) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>{ui}</MemoryRouter>,
  );
}

describe('AppLayout', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    setupMatchMedia();
  });

  it('renders header with app name', () => {
    renderWithRouter(<AppLayout projectsData={{ data: [], total: 0 }} />);

    expect(screen.getByText('KanPad')).toBeInTheDocument();
  });

  it('renders user email in header', () => {
    renderWithRouter(<AppLayout projectsData={{ data: [], total: 0 }} />);

    expect(screen.getByText('test@example.com')).toBeInTheDocument();
  });

  it('renders theme toggle button', () => {
    renderWithRouter(<AppLayout projectsData={{ data: [], total: 0 }} />);

    expect(screen.getByLabelText('Toggle theme')).toBeInTheDocument();
  });

  it('renders Outlet for nested routes', () => {
    renderWithRouter(<AppLayout projectsData={{ data: [], total: 0 }} />);

    expect(screen.getByTestId('outlet')).toBeInTheDocument();
  });

  it('renders BoardNotesSidebar when boardId is in route', () => {
    render(
      <MemoryRouter initialEntries={['/boards/42']}>
        <Routes>
          <Route path="/boards/:boardId" element={<AppLayout projectsData={{ data: [], total: 0 }} />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByTestId('board-notes-sidebar')).toBeInTheDocument();
    expect(screen.getByText('Board Notes Sidebar 42')).toBeInTheDocument();
  });

  it('does not render BoardNotesSidebar when boardId is not in route', () => {
    render(
      <MemoryRouter initialEntries={['/']}>
        <Routes>
          <Route path="/" element={<AppLayout projectsData={{ data: [], total: 0 }} />} />
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.queryByTestId('board-notes-sidebar')).not.toBeInTheDocument();
  });
});
