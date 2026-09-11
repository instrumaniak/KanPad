import { useState } from 'react';
import { ColumnCardList } from './column-card-list';
import { ColumnHeader } from './column-header';
import { ColumnDroppable } from './column-droppable';
import { AddCardInput } from '../cards/add-card-input';
import { type Column as ColumnType } from './use-columns';
import type { Card as CardType2 } from '../cards/use-cards';
import { useBreakpoint } from '@/hooks/use-breakpoint';

interface ColumnProps {
  column: ColumnType;
  allColumns?: ColumnType[];
  onDeleted?: () => void;
}

export function Column({ column, allColumns = [], onDeleted }: ColumnProps) {
  const [newCardId, setNewCardId] = useState<number | undefined>();
  const breakpoint = useBreakpoint();
  const nextColumn = allColumns.find((c) => c.position === column.position + 1);
  const nextColumnId = nextColumn?.id;

  const handleCardCreated = (card: CardType2) => {
    setNewCardId(card.id);
    setTimeout(() => setNewCardId(undefined), 500);
  };

  const isMobile = breakpoint === 'mobile';

  return (
    <div
      className={`flex h-full shrink-0 flex-col rounded-lg ${isMobile ? 'w-[17rem] max-w-[calc(100vw-2rem)]' : 'min-w-[320px] max-w-[320px]'}`}
      data-column-id={column.id}
    >
      <ColumnHeader column={column} allColumns={allColumns} onDeleted={onDeleted} />

      <ColumnDroppable columnId={column.id}>
        {({ isOver }) => (
          <div
            className={`flex-1 overflow-y-auto p-2 transition-colors ${
              isOver ? 'bg-teal-500/10' : ''
            }`}
          >
            <ColumnCardList cards={column.cards as CardType2[]} newCardId={newCardId} />
          </div>
        )}
      </ColumnDroppable>

      <div className="p-2">
        <AddCardInput columnId={column.id} nextColumnId={nextColumnId} onCardCreated={handleCardCreated} />
      </div>
    </div>
  );
}
