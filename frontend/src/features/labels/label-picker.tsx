import { useState } from 'react';
import type { Label, Card, LabelColor } from '../cards/cards.api';
import { useLabels, useCreateLabel } from './use-labels';
import { useAssignCardLabel, useRemoveCardLabel } from '../cards/use-cards';
import { useToastHelpers } from '@/lib/toast-helpers';
import { Plus } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { LABEL_COLOR_OPTIONS, getLabelColorClass } from './label-colors';

interface LabelPickerProps {
  card: Card;
}

export function LabelPicker({ card }: LabelPickerProps) {
  const { data: labels, isLoading } = useLabels();
  const assignLabel = useAssignCardLabel();
  const removeLabel = useRemoveCardLabel();
  const createLabel = useCreateLabel();
  const { showSuccess, showError } = useToastHelpers();
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState('');
  const [newColor, setNewColor] = useState<LabelColor>('blue');

  const cardLabelIds = new Set(card.labels?.map((l) => l.id) ?? []);

  const handleToggle = (label: Label) => {
    if (cardLabelIds.has(label.id)) {
      removeLabel.mutate(
        { cardId: card.id, labelId: label.id },
        {
          onError: () => {
            showError('Failed to remove label');
          },
        },
      );
    } else {
      assignLabel.mutate(
        { cardId: card.id, labelId: label.id },
        {
          onError: () => {
            showError('Failed to assign label');
          },
        },
      );
    }
  };

  const handleCreate = () => {
    const trimmed = newName.trim();
    if (!trimmed) return;
    createLabel.mutate(
      { name: trimmed, color: newColor },
      {
        onSuccess: () => {
          setNewName('');
          setNewColor('blue');
          setIsCreating(false);
          showSuccess('Label created');
        },
        onError: () => {
          showError('Failed to create label');
        },
      },
    );
  };

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline" size="sm" className="h-8 text-xs">
          {card.labels && card.labels.length > 0
            ? `${card.labels.length} label${card.labels.length > 1 ? 's' : ''}`
            : 'Add labels'}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className="w-64 max-h-[var(--radix-popper-available-height)] overflow-y-auto"
        align="start"
      >
        {isLoading ? (
          <p className="text-muted-foreground text-sm">Loading labels...</p>
        ) : !labels || labels.length === 0 ? (
          <div className="space-y-2">
            <p className="text-muted-foreground text-sm">No labels available.</p>
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              onClick={() => setIsCreating(true)}
            >
              <Plus className="mr-1 h-3 w-3" />
              Create Label
            </Button>
          </div>
        ) : (
          <div className="space-y-2">
            <div className="flex flex-wrap gap-1.5">
              {labels.map((label) => {
                const isAssigned = cardLabelIds.has(label.id);
                const colorClass = getLabelColorClass(label.color);
                return (
                  <button
                    key={label.id}
                    type="button"
                    onClick={() => handleToggle(label)}
                    aria-label={isAssigned ? `Remove ${label.name}` : `Add ${label.name}`}
                    aria-pressed={isAssigned}
                    role="button"
                    className={`inline-flex items-center rounded px-2 py-0.5 text-xs font-medium transition-opacity hover:opacity-80 focus:outline-none focus:ring-2 focus:ring-ring ${colorClass} ${
                      isAssigned ? 'opacity-100 ring-2 ring-ring' : 'opacity-50'
                    }`}
                    title={isAssigned ? `Remove ${label.name}` : `Add ${label.name}`}
                    disabled={assignLabel.isPending || removeLabel.isPending}
                  >
                    {label.name}
                  </button>
                );
              })}
            </div>
            {!isCreating && (
              <Button
                variant="ghost"
                size="sm"
                className="h-7 w-full text-xs justify-start"
                onClick={() => setIsCreating(true)}
              >
                <Plus className="mr-1 h-3 w-3" />
                Create new label
              </Button>
            )}
          </div>
        )}
        {isCreating && (
          <div className="mt-2 space-y-2 border-t pt-2">
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="Label name"
              className="w-full rounded border bg-background px-2 py-1 text-sm outline-none focus:ring-2 focus:ring-ring"
              maxLength={50}
              autoFocus
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleCreate();
                if (e.key === 'Escape') setIsCreating(false);
              }}
            />
            <div className="flex gap-1">
              {LABEL_COLOR_OPTIONS.map((c) => (
                <button
                  key={c.value}
                  type="button"
                  onClick={() => setNewColor(c.value)}
                  aria-label={c.label}
                  className={`h-5 w-5 rounded-full ${c.className} ${newColor === c.value ? 'ring-2 ring-ring ring-offset-1' : ''}`}
                />
              ))}
            </div>
            <div className="flex gap-1">
              <Button
                size="sm"
                className="h-7 text-xs"
                onClick={handleCreate}
                disabled={!newName.trim() || createLabel.isPending}
              >
                {createLabel.isPending ? 'Creating...' : 'Create'}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 text-xs"
                onClick={() => {
                  setIsCreating(false);
                  setNewName('');
                }}
              >
                Cancel
              </Button>
            </div>
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
