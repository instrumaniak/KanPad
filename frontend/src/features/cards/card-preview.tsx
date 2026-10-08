import { useMemo } from 'react';
import { Letters } from 'lucide-react';
import type { Card as CardType } from './use-cards';
import type { ReactNode } from 'react';
import { LabelBadge } from '../labels/label-badge';
import { getDueDateBadge } from './date-utils';
import { ProgressBar } from '../checklists/progress-bar';

interface CardPreviewProps {
  card: CardType;
  actions?: ReactNode;
}

export function CardPreview({ card, actions }: CardPreviewProps) {
  const dueDateBadge = useMemo(() => getDueDateBadge(card.due_date), [card.due_date]);
  const hasCornerContent = card.has_description || !!actions;

  return (
    <>
      <div className={`relative flex items-start gap-2${hasCornerContent ? ' pr-6' : ''}`}>
        <span className="flex-1 break-words">{card.title}</span>
        {hasCornerContent && (
          <div className="absolute top-0 right-0 flex items-start gap-1">
            {card.has_description && (
              <Letters
                role="img"
                className="h-4 w-4 shrink-0 text-muted-foreground"
                aria-label="Has description"
                data-testid="card-description-icon"
              />
            )}
            {actions ? <div className="shrink-0">{actions}</div> : null}
          </div>
        )}
      </div>

      {card.labels && card.labels.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1">
          {card.labels.map((label) => (
            <LabelBadge key={label.id} label={label} />
          ))}
        </div>
      )}

      {dueDateBadge && (
        <span
          className={`mt-2 inline-flex items-center rounded px-2 py-0.5 text-xs font-medium ${dueDateBadge.className}`}
          aria-label={`Due date: ${dueDateBadge.text}`}
        >
          {dueDateBadge.text}
        </span>
      )}

      {card.checklist_progress && (
        <div className="mt-2 flex items-center gap-2">
          <ProgressBar
            completed={card.checklist_progress.completed}
            total={card.checklist_progress.total}
            className="flex-1"
          />
          <span className="text-xs text-muted-foreground whitespace-nowrap">
            {card.checklist_progress.completed}/{card.checklist_progress.total} (
            {card.checklist_progress.percent}%)
          </span>
        </div>
      )}
    </>
  );
}
