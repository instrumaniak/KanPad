import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { CardDraggable } from './card-draggable';

const mockUseSortable = vi.fn();

vi.mock('@dnd-kit/sortable', () => ({
  useSortable: (...args: unknown[]) => mockUseSortable(...args),
}));

vi.mock('@dnd-kit/utilities', () => ({
  CSS: { Transform: { toString: () => '' } },
}));

const mockCard = {
  id: 1,
  title: 'Test Card',
  column_id: 1,
  position: 0,
  description: null,
  due_date: null,
  created_at: '2024-01-01',
  updated_at: '2024-01-01',
};

describe('CardDraggable', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: vi.fn().mockImplementation((query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
      })),
    });
    vi.clearAllMocks();
    mockUseSortable.mockReturnValue({
      attributes: {},
      listeners: {},
      setNodeRef: vi.fn(),
      transform: null,
      transition: null,
      isDragging: false,
    });
  });

  it('renders children via render prop function', () => {
    render(
      <CardDraggable card={mockCard} index={0}>
        {() => <div data-testid="child">Content</div>}
      </CardDraggable>,
    );
    expect(screen.getByTestId('child')).toHaveTextContent('Content');
  });

  it('passes correct render prop arguments from useSortable', () => {
    const renderSpy = vi.fn().mockReturnValue(<div data-testid="child" />);
    render(
      <CardDraggable card={mockCard} index={0}>
        {renderSpy}
      </CardDraggable>,
    );
    expect(renderSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        isDragging: false,
        transform: null,
        attributes: {},
        listeners: {},
        setNodeRef: expect.any(Function),
      }),
    );
  });

  it('passes card data and index to useSortable', () => {
    render(
      <CardDraggable card={mockCard} index={2}>
        {() => <div data-testid="child" />}
      </CardDraggable>,
    );
    expect(mockUseSortable).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'card-1',
        disabled: false,
        data: expect.objectContaining({
          cardId: 1,
          card: mockCard,
          index: 2,
        }),
      }),
    );
  });

  it('does not spread listeners and attributes on outer div when isDragDisabled is true', () => {
    mockUseSortable.mockReturnValueOnce({
      attributes: { 'data-dnd-enabled': 'true' },
      listeners: { onClick: vi.fn() },
      setNodeRef: vi.fn(),
      transform: null,
      transition: null,
      isDragging: false,
    });

    render(
      <CardDraggable card={mockCard} index={0} isDragDisabled={true}>
        {() => <div data-testid="child" />}
      </CardDraggable>,
    );

    const outerDiv = screen.getByTestId('child').parentElement!;
    expect(outerDiv.hasAttribute('data-dnd-enabled')).toBe(false);
  });

  it('spreads listeners and attributes on outer div when isDragDisabled is false', () => {
    mockUseSortable.mockReturnValueOnce({
      attributes: { 'data-dnd-enabled': 'true' },
      listeners: { onClick: vi.fn() },
      setNodeRef: vi.fn(),
      transform: null,
      transition: null,
      isDragging: false,
    });

    render(
      <CardDraggable card={mockCard} index={0}>
        {() => <div data-testid="child" />}
      </CardDraggable>,
    );

    const outerDiv = screen.getByTestId('child').parentElement!;
    expect(outerDiv.getAttribute('data-dnd-enabled')).toBe('true');
  });

  it('shows dragging styles when isDragging is true', () => {
    mockUseSortable.mockReturnValue({
      attributes: {},
      listeners: {},
      setNodeRef: vi.fn(),
      transform: null,
      transition: null,
      isDragging: true,
    });

    render(
      <CardDraggable card={mockCard} index={0}>
        {({ isDragging }) => (
          <div data-testid="child" data-is-dragging={isDragging} />
        )}
      </CardDraggable>,
    );

    const child = screen.getByTestId('child');
    expect(child).toHaveAttribute('data-is-dragging', 'true');

    const outerDiv = child.parentElement!;
    expect(outerDiv.style.opacity).toBe('0.5');
    expect(outerDiv.style.cursor).toBe('grabbing');
  });

  it('shows default pointer cursor when not dragging, not hovered, and not disabled', () => {
    render(
      <CardDraggable card={mockCard} index={0}>
        {() => <div data-testid="child" />}
      </CardDraggable>,
    );

    const outerDiv = screen.getByTestId('child').parentElement!;
    expect(outerDiv.style.cursor).toBe('pointer');
  });

  it('shows grab cursor on hover when not dragging', () => {
    render(
      <CardDraggable card={mockCard} index={0}>
        {() => <div data-testid="child" />}
      </CardDraggable>,
    );

    const outerDiv = screen.getByTestId('child').parentElement!;
    fireEvent.mouseEnter(outerDiv);
    expect(outerDiv.style.cursor).toBe('grab');

    fireEvent.mouseLeave(outerDiv);
    expect(outerDiv.style.cursor).toBe('pointer');
  });

  it('shows default cursor when isDragDisabled even when hovered', () => {
    render(
      <CardDraggable card={mockCard} index={0} isDragDisabled={true}>
        {() => <div data-testid="child" />}
      </CardDraggable>,
    );

    const outerDiv = screen.getByTestId('child').parentElement!;
    expect(outerDiv.style.cursor).toBe('default');

    fireEvent.mouseEnter(outerDiv);
    expect(outerDiv.style.cursor).toBe('default');
  });
});
