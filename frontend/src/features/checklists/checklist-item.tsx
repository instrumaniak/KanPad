import { useState, useRef } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { Checkbox } from '@/components/ui/checkbox';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useUpdateChecklistItem, useDeleteChecklistItem } from './use-checklists';
import type { ChecklistItem as ChecklistItemType } from './checklists.api';
import { createChecklistItem, updateChecklistItem } from './checklists.api';
import { useToast } from '@/components/ui/use-toast';
import { useToastHelpers } from '@/lib/toast-helpers';

interface ChecklistItemProps {
  item: ChecklistItemType;
  cardId: number;
}

export function ChecklistItem({ item, cardId }: ChecklistItemProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [text, setText] = useState(item.text);
  const inputRef = useRef<HTMLInputElement>(null);
  const { toast } = useToast();
  const { showError } = useToastHelpers();

  const queryClient = useQueryClient();
  const updateMutation = useUpdateChecklistItem();
  const deleteMutation = useDeleteChecklistItem();

  const handleToggle = (checked: boolean) => {
    updateMutation.mutate({
      itemId: item.id,
      data: { is_completed: checked },
      cardId,
    });
  };

  const handleSave = () => {
    if (text.trim() === '') {
      setText(item.text);
      setIsEditing(false);
      return;
    }
    if (text.trim() !== item.text.trim()) {
      updateMutation.mutate(
        {
          itemId: item.id,
          data: { text: text.trim() },
          cardId,
        },
        {
          onError: () => {
            setText(item.text);
            showError('Failed to save item text');
          },
        },
      );
    }
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSave();
    } else if (e.key === 'Escape') {
      setText(item.text);
      setIsEditing(false);
    }
  };

  const handleDelete = () => {
    const previousItem = { ...item };
    deleteMutation.mutate({ itemId: item.id, cardId }, {
      onSuccess: () => {
          toast({
            title: 'Item deleted',
          action: {
            label: 'Undo',
            onClick: async () => {
              try {
                const restored = await createChecklistItem(previousItem.checklist_id, {
                  text: previousItem.text,
                  position: previousItem.position,
                });

                if (previousItem.is_completed) {
                  await updateChecklistItem(restored.data.id, { is_completed: true });
                }

                queryClient.invalidateQueries({ queryKey: ['card', cardId] });
                queryClient.invalidateQueries({ queryKey: ['columns'] });
              } catch (error) {
                const message = error instanceof Error ? error.message : 'Unknown error';
                showError('Failed to undo delete', message);
              }
            },
          },
        });
      },
    });
  };

  return (
    <div className="flex items-center gap-2 group py-1">
      <Checkbox
        checked={item.is_completed}
        onCheckedChange={handleToggle}
        className="flex-shrink-0"
      />
      {isEditing ? (
        <Input
          ref={inputRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onBlur={handleSave}
          onKeyDown={handleKeyDown}
          className="h-6 text-sm flex-1"
          autoFocus
        />
      ) : (
        <span
          className={`flex-1 text-sm cursor-pointer ${
            item.is_completed ? 'line-through text-muted-foreground' : ''
          }`}
          onClick={() => {
            setText(item.text);
            setIsEditing(true);
          }}
        >
          {item.text}
        </span>
      )}
      <Button
        variant="ghost"
        size="sm"
        className="opacity-0 group-hover:opacity-100 focus-within:opacity-100 h-6 w-6 p-0"
        onClick={handleDelete}
      >
        <span className="sr-only">Delete</span>
        <span className="text-muted-foreground">×</span>
      </Button>
    </div>
  );
}
