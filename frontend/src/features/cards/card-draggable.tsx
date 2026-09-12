import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { CSSProperties } from 'react';
import { type Card as CardType, type DragData } from './use-cards';
import { useState } from 'react';
import { usePrefersReducedMotion } from '@/hooks/use-prefers-reduced-motion';

interface CardDraggableProps {
  card: CardType;
  index: number;
  isDragDisabled?: boolean;
  children: (props: {
    isDragging: boolean;
    transform: CSSProperties['transform'] | null;
    attributes: Record<string, unknown>;
    listeners: Record<string, unknown>;
    setNodeRef: (node: HTMLElement | null) => void;
  }) => React.ReactNode;
}

export function CardDraggable({ card, index, isDragDisabled = false, children }: CardDraggableProps) {
  const [isHovered, setIsHovered] = useState(false);
  const prefersReducedMotion = usePrefersReducedMotion();
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id: `card-${card.id}`,
    disabled: isDragDisabled,
    data: {
      cardId: card.id,
      sourceColumnId: card.column_id ?? undefined,
      card: card,
      index,
    } as DragData & { index: number },
    transition: prefersReducedMotion
      ? { duration: 1, easing: 'linear' }
      : {
          duration: 250,
          easing: 'cubic-bezier(0.25, 1, 0.5, 1)',
        },
  });

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  };

  return (
    <div
      ref={setNodeRef}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      style={{
        ...style,
        boxShadow: isDragging
          ? '0 10px 20px rgba(0,0,0,0.15)'
          : isHovered
          ? '0 4px 6px rgba(0,0,0,0.1)'
          : '0 1px 3px rgba(0,0,0,0.1)',
        cursor: isDragDisabled ? 'default' : isDragging ? 'grabbing' : isHovered ? 'grab' : 'pointer',
      }}
      {...(isDragDisabled ? {} : listeners)}
      {...(isDragDisabled ? {} : { ...attributes })}
    >
      {children({
        isDragging,
        transform: transform ? CSS.Transform.toString(transform) : null,
        attributes: { ...attributes } as Record<string, unknown>,
        listeners: { ...listeners } as Record<string, unknown>,
        setNodeRef,
      })}
    </div>
  );
}