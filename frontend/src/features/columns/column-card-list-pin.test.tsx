import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { ColumnCardList } from './column-card-list';
import type { Card } from '../cards/use-cards';

// Simulate an in-flight drag: useDndContext returns an active draggable with id
// 'card-3'. The main column-card-list.test.tsx uses the default (no provider)
// context — pinning behavior needs an active drag.
const dndState = vi.hoisted(() => ({
  active: { id: 'card-3' } as { id: string | number } | null,
}));

vi.mock('@dnd-kit/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/core')>();
  return {
    ...actual,
    useDndContext: () => ({ active: dndState.active }),
  };
});

// Keep the DOM light but focusable — the Card component's internals are
// covered by card.test.tsx.
vi.mock('../cards/card', () => ({
  Card: ({ card, index }: { card: { id: number; title: string }; index: number }) => (
    <button data-testid="card" data-card-index={index} type="button">
      {card.title}
    </button>
  ),
}));

const makeCard = (id: number): Card => ({
  id,
  title: `Card ${id}`,
  column_id: 1,
  position: id,
  has_description: false,
  labels: [],
  due_date: null,
  checklist_progress: null,
  created_at: '2026-01-01T00:00:00.000Z',
  updated_at: '2026-01-01T00:00:00.000Z',
});

const makeCards = (n: number) => Array.from({ length: n }, (_, i) => makeCard(i + 1));

describe('ColumnCardList — dragged row pinning (T021)', () => {
  const cards = makeCards(300);

  beforeEach(() => {
    Object.defineProperty(Element.prototype, 'clientHeight', { configurable: true, value: 600 });
    Object.defineProperty(Element.prototype, 'clientWidth', { configurable: true, value: 320 });
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 140 });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, value: 320 });
    dndState.active = { id: 'card-3' };
  });
  afterEach(() => {
    Object.defineProperty(Element.prototype, 'clientHeight', { configurable: true, value: 0 });
    Object.defineProperty(Element.prototype, 'clientWidth', { configurable: true, value: 0 });
    Object.defineProperty(HTMLElement.prototype, 'offsetHeight', { configurable: true, value: 0 });
    Object.defineProperty(HTMLElement.prototype, 'offsetWidth', { configurable: true, value: 0 });
  });

  it('keeps the dragged source row mounted when the list scrolls far past it', () => {
    const { container } = render(<ColumnCardList cards={cards} />);
    expect(container.querySelector('[data-index="2"]')).not.toBeNull();

    const scroller = container.querySelector('[data-testid="column-card-list-scroll"]') as HTMLElement;
    scroller.scrollTop = 140 * 250;
    scroller.dispatchEvent(new Event('scroll'));

    // The window has shifted to rows ~248-258, but the dragged row (index 2)
    // stays mounted via the draggedIndex pin in rangeExtractor.
    expect(container.querySelector('[data-index="2"]')).not.toBeNull();
  });

  it('renders the dragged+focused row exactly once when both pins coincide (keyboard drag)', () => {
    // Regression: during a keyboard drag the source card is both the dragged and
    // the focused card — the rangeExtractor union must not duplicate the row.
    // Scroll far down so the pinned index is OUTSIDE the base window — only then
    // does the union produce the duplicate.
    const { container } = render(<ColumnCardList cards={makeCards(300)} />);
    const target = container.querySelector('[data-index="2"] button')!;
    fireEvent.focusIn(target);

    const scroller = container.querySelector('[data-testid="column-card-list-scroll"]') as HTMLElement;
    scroller.scrollTop = 140 * 250;
    scroller.dispatchEvent(new Event('scroll'));

    expect(container.querySelectorAll('[data-index="2"]')).toHaveLength(1);
  });

  it('unpins a focused card once focus moves outside it (row virtualizes away)', () => {
    // No active drag here — only the focused-row pin is in play.
    dndState.active = null;
    const { container } = render(<ColumnCardList cards={makeCards(300)} />);
    const target = container.querySelector('[data-index="2"] button')!;
    fireEvent.focusIn(target);

    // Blur to an element OUTSIDE the row — the pin must release.
    const outside = document.createElement('button');
    outside.textContent = 'outside';
    container.appendChild(outside);
    fireEvent.blur(target, { relatedTarget: outside });

    const scroller = container.querySelector('[data-testid="column-card-list-scroll"]') as HTMLElement;
    scroller.scrollTop = 140 * 250;
    scroller.dispatchEvent(new Event('scroll'));

    expect(container.querySelector('[data-index="2"]')).toBeNull();
    container.removeChild(outside);
  });

  it('drops the dragged-row pin once the drag ends', () => {
    const { container, rerender } = render(<ColumnCardList cards={cards} />);
    const scroller = container.querySelector('[data-testid="column-card-list-scroll"]') as HTMLElement;
    scroller.scrollTop = 140 * 250;
    scroller.dispatchEvent(new Event('scroll'));
    expect(container.querySelector('[data-index="2"]')).not.toBeNull();

    // Drag ends: no active node → pin must not force-mount the row anymore.
    dndState.active = null;
    rerender(<ColumnCardList cards={cards} />);
    expect(container.querySelector('[data-index="2"]')).toBeNull();
  });
});
