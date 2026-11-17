import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { CardPreview } from './card-preview';

describe('CardPreview', () => {
  const baseCard = {
    id: 1,
    title: 'Preview Card',
    column_id: 1,
    position: 0,
    has_description: false,
    due_date: null as string | null,
    labels: [
      { id: 1, name: 'Bug', color: 'red', created_at: '2024-01-01', updated_at: '2024-01-01' },
      { id: 2, name: 'Feature', color: 'green', created_at: '2024-01-01', updated_at: '2024-01-01' },
      { id: 3, name: 'Urgent', color: 'orange', created_at: '2024-01-01', updated_at: '2024-01-01' },
      { id: 4, name: 'Important', color: 'blue', created_at: '2024-01-01', updated_at: '2024-01-01' },
    ],
    created_at: '2024-01-01',
    updated_at: '2024-01-01',
  };

  let dateNowSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    // Mock Date.now() and new Date() to return a fixed date for consistent testing
    const fixedDate = new Date('2026-06-15T12:00:00.000Z');
    dateNowSpy = vi.spyOn(Date, 'now').mockReturnValue(fixedDate.getTime());
    vi.useFakeTimers();
    vi.setSystemTime(fixedDate);
  });

  afterEach(() => {
    vi.useRealTimers();
    dateNowSpy.mockRestore();
  });

  it('renders the shared card body content with all labels', () => {
    render(<CardPreview card={{ ...baseCard, due_date: null }} />);

    expect(screen.getByText('Preview Card')).toBeInTheDocument();
    expect(screen.getByText('Bug')).toBeInTheDocument();
    expect(screen.getByText('Feature')).toBeInTheDocument();
    expect(screen.getByText('Urgent')).toBeInTheDocument();
    expect(screen.getByText('Important')).toBeInTheDocument();
    expect(screen.queryByText(/\+\d+/)).not.toBeInTheDocument();
  });

  it('shows description icon when has_description is true', () => {
    render(<CardPreview card={{ ...baseCard, due_date: null, has_description: true }} />);

    expect(screen.getByLabelText('Has description')).toBeInTheDocument();
    expect(screen.getByTestId('card-description-icon')).toBeInTheDocument();
  });

  it('hides description icon when has_description is false', () => {
    render(<CardPreview card={{ ...baseCard, due_date: null, has_description: false }} />);

    expect(screen.queryByLabelText('Has description')).not.toBeInTheDocument();
    expect(screen.queryByTestId('card-description-icon')).not.toBeInTheDocument();
  });

  it('pins description icon and actions top-right of the title row', () => {
    const { container } = render(
      <CardPreview
        card={{ ...baseCard, due_date: null, has_description: true }}
        actions={<button type="button">Menu</button>}
      />,
    );

    const header = container.firstChild as HTMLElement | null;
    expect(header?.className).toContain('relative');
    expect(header?.className).toContain('pr-6');

    const icon = screen.getByTestId('card-description-icon');
    const cluster = icon.closest('div');
    expect(cluster?.className).toContain('absolute');
    expect(cluster?.className).toContain('right-0');
    expect(cluster?.className).toContain('top-0');
    expect(cluster?.textContent).toContain('Menu');
  });

  it('renders optional actions beside the title', () => {
    render(
      <CardPreview
        card={{ ...baseCard, due_date: null }}
        actions={<button type="button">Menu</button>}
      />
    );

    expect(screen.getByRole('button', { name: 'Menu' })).toBeInTheDocument();
  });

  it('shows "Today" for cards due today', () => {
    render(<CardPreview card={{ ...baseCard, due_date: '2026-06-15T00:00:00.000Z' }} />);

    expect(screen.getByText('Today')).toBeInTheDocument();
    expect(screen.getByLabelText('Due date: Today')).toBeInTheDocument();
  });

  it('shows formatted date for past due dates', () => {
    render(<CardPreview card={{ ...baseCard, due_date: '2026-06-10T00:00:00.000Z' }} />);

    expect(screen.getByText('Jun 10, 2026')).toBeInTheDocument();
    expect(screen.getByLabelText('Due date: Jun 10, 2026')).toBeInTheDocument();
  });

  it('shows formatted date for future due dates', () => {
    render(<CardPreview card={{ ...baseCard, due_date: '2026-06-20T00:00:00.000Z' }} />);

    expect(screen.getByText('Jun 20, 2026')).toBeInTheDocument();
    expect(screen.getByLabelText('Due date: Jun 20, 2026')).toBeInTheDocument();
  });

  it('does not show due date badge when due_date is null', () => {
    render(<CardPreview card={{ ...baseCard, due_date: null }} />);

    expect(screen.queryByLabelText(/Due date:/)).not.toBeInTheDocument();
  });

  it('does not show due date badge for invalid date string', () => {
    render(<CardPreview card={{ ...baseCard, due_date: 'invalid-date' }} />);

    expect(screen.queryByLabelText(/Due date:/)).not.toBeInTheDocument();
  });

  it('renders checklist progress with complete state', () => {
    render(
      <CardPreview
        card={{
          ...baseCard,
          due_date: null,
          checklist_progress: { completed: 2, total: 2, percent: 100 },
        }}
      />,
    );

    expect(screen.getByText('2/2 (100%)')).toBeInTheDocument();
    expect(screen.getByLabelText('Checklist progress: 2/2 (100%)')).toHaveAttribute(
      'data-progress-state',
      'complete',
    );
  });

  it('renders aggregated checklist progress from the api response', () => {
    render(
      <CardPreview
        card={{
          ...baseCard,
          due_date: null,
          checklist_progress: { completed: 3, total: 4, percent: 75 },
        }}
      />,
    );

    expect(screen.getByText('3/4 (75%)')).toBeInTheDocument();
    expect(screen.getByLabelText('Checklist progress: 3/4 (75%)')).toHaveAttribute(
      'data-progress-state',
      'partial',
    );
  });

  it('has accessible due date badge with proper aria-label', () => {
    render(<CardPreview card={{ ...baseCard, due_date: '2026-06-15T00:00:00.000Z' }} />);

    const badge = screen.getByLabelText('Due date: Today');
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveAttribute('aria-label', 'Due date: Today');
  });
});

describe('CardPreview markdown title', () => {
  const baseCard = {
    id: 1,
    title: 'Preview Card',
    column_id: 1,
    position: 0,
    has_description: false,
    due_date: null as string | null,
    labels: [],
    created_at: '2024-01-01',
    updated_at: '2024-01-01',
  };

  it('renders basic markdown in the card title (bold/emphasis/code)', () => {
    render(
      <CardPreview
        card={{ ...baseCard, title: '**Fix** the `login` *bug*', due_date: null }}
      />,
    );
    expect(screen.getByText('Fix').tagName).toBe('STRONG');
    expect(screen.getByText('login').tagName).toBe('CODE');
    expect(screen.getByText('bug').tagName).toBe('EM');
  });

  it('filters javascript: URLs in title links (sanitize)', () => {
    render(
      <CardPreview
        card={{ ...baseCard, title: '[click](javascript:alert(1))', due_date: null }}
      />,
    );
    // the unsafe link is not rendered as a link at all; it renders as inert text
    expect(document.querySelector('a')).toBeNull();
    expect(document.querySelector('[href]')).toBeNull();
  });

  it('renders http(s) links in title with target=_blank and rel=noopener', () => {
    render(
      <CardPreview
        card={{ ...baseCard, title: '[docs](https://example.com/spec)', due_date: null }}
      />,
    );
    const link = screen.getByRole('link', { name: 'docs' });
    expect(link).toHaveAttribute('href', 'https://example.com/spec');
    expect(link).toHaveAttribute('target', '_blank');
    expect(link).toHaveAttribute('rel', expect.stringContaining('noopener'));
  });

  it('renders raw HTML in the title as escaped text, never as markup', () => {
    render(
      <CardPreview
        card={{ ...baseCard, title: '<img src=x onerror=alert(1)> broken', due_date: null }}
      />,
    );
    expect(screen.queryByRole('img')).toBeNull();
    expect(screen.getByText(/broken/)).toBeInTheDocument();
  });
});
