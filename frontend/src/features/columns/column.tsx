import { memo, useState } from 'react';
import { ColumnCardList } from './column-card-list';
import { ColumnHeader } from './column-header';
import { ColumnDroppable } from './column-droppable';
import { AddCardInput } from '../cards/add-card-input';
import { type Column as ColumnType } from './use-columns';
import type { Card as CardType2 } from '../cards/use-cards';
import { useBreakpoint } from '@/hooks/use-breakpoint';

interface ColumnProps {
  column: ColumnType;
  boardId: number;
  nextColumnId?: number;
  moveTargets?: Array<Pick<ColumnType, 'id' | 'name'>>;
  onDeleted?: () => void;
}

export const Column = memo(function Column({ column, boardId, nextColumnId, moveTargets = [], onDeleted }: ColumnProps) {
  const [newCardId, setNewCardId] = useState<number | undefined>();
  const breakpoint = useBreakpoint();

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
      <ColumnHeader column={column} moveTargets={moveTargets} onDeleted={onDeleted} />

      {/* Layout contract (button placement): the scroller is content-sized —
          NO flex-1 — so it only takes as much height as the cards need and
          "+ Add a card" rides right below the content. On overflow the scroller
          alone shrinks (min-h-0), pinning the footer to the bottom of the
          column's *visible* height. The footer lives INSIDE the droppable so
          the drop zone keeps the full column body below the header (empty
          columns would otherwise collapse their drop target to a stub). */}
      <ColumnDroppable columnId={column.id}>
        {({ isOver }) => (
          <>
            <div
              className={`flex min-h-0 flex-col transition-colors ${
                isOver ? 'bg-teal-500/10' : ''
              }`}
            >
              <ColumnCardList
                cards={column.cards}
                columnId={column.id}
                boardId={boardId}
                totalCards={(column as ColumnType & { total_cards?: number }).total_cards}
                newCardId={newCardId}
              />
            </div>
            <div className="p-2">
              <AddCardInput columnId={column.id} nextColumnId={nextColumnId} onCardCreated={handleCardCreated} />
            </div>
          </>
        )}
      </ColumnDroppable>
    </div>
  );
});
