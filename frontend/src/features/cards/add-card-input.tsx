import { useState, useRef, type KeyboardEvent, type FocusEvent, type ChangeEvent } from 'react';
import { useCreateCard, type Card } from './use-cards';
import { useToastHelpers } from '@/lib/toast-helpers';

interface AddCardInputProps {
  columnId: number;
  nextColumnId?: number;
  onCardCreated?: (card: Card) => void;
  onCardCreateError?: (error: Error) => void;
}

export function AddCardInput({ columnId, nextColumnId, onCardCreated, onCardCreateError }: AddCardInputProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [title, setTitle] = useState('');
  const [isCreating, setIsCreating] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const createCard = useCreateCard();
  const { showError } = useToastHelpers();

  const adjustTextareaHeight = (textarea: HTMLTextAreaElement) => {
    textarea.style.height = 'auto';
    textarea.style.height = `${Math.min(textarea.scrollHeight, 120)}px`;
  };

  const openInput = () => setIsOpen(true);

  const closeInput = () => {
    setIsOpen(false);
    setTitle('');
  };

  const handleKeyDown = async (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      if (title.trim()) {
        const trimmedTitle = title.trim();
        setIsCreating(true);
        
        const optimisticCard: Card = {
          id: Date.now(),
          title: trimmedTitle,
          column_id: columnId,
          position: 0,
          description: null,
          due_date: null,
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        };
        
        onCardCreated?.(optimisticCard);
        
        try {
          await createCard.mutateAsync({
            title: trimmedTitle,
            column_id: columnId,
          });
          setTitle('');
        } catch (err) {
          const error = err instanceof Error ? err : new Error('Failed to create card');
          onCardCreateError?.(error);
          showError('Failed to create card', error.message);
        } finally {
          setIsCreating(false);
          inputRef.current?.focus();
        }
      } else {
        closeInput();
      }
    } else if (e.key === 'Escape') {
      e.preventDefault();
      closeInput();
    } else if (e.key === 'Tab' && nextColumnId && !isCreating) {
      e.preventDefault();
      if (title.trim()) {
        setIsCreating(true);
        try {
          await createCard.mutateAsync({
            title: title.trim(),
            column_id: columnId,
          });
        } catch (err) {
          const error = err instanceof Error ? err : new Error('Failed to create card');
          showError('Failed to create card', error.message);
        } finally {
          setIsCreating(false);
        }
      }
      const selector = `[data-column-id="${nextColumnId}"] textarea`;
      const nextInput = document.querySelector(selector) as HTMLTextAreaElement | null;
      if (nextInput) {
        nextInput.focus();
      } else {
        closeInput();
      }
    }
  };

  const handleChange = (e: ChangeEvent<HTMLTextAreaElement>) => {
    setTitle(e.target.value);
    adjustTextareaHeight(e.target);
  };

  const handleBlur = (e: FocusEvent<HTMLTextAreaElement>) => {
    const relatedTarget = e.relatedTarget as HTMLElement | null;
    if (!relatedTarget?.closest(`[data-column-id="${columnId}"]`)) {
      if (!title.trim()) {
        closeInput();
      }
    }
  };

  if (!isOpen) {
    return (
      <button
        type="button"
        onClick={openInput}
        className="flex w-full items-center rounded px-2 py-1.5 text-sm text-muted-foreground hover:bg-accent"
      >
        + Add a card
      </button>
    );
  }

  return (
    <div>
      <textarea
        ref={inputRef}
        value={title}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        onBlur={handleBlur}
        placeholder="Enter a title..."
        rows={1}
        autoFocus
        className="w-full resize-none rounded bg-card p-2 text-sm shadow-sm outline-none focus:ring-2 focus:ring-primary"
        style={{ minHeight: '2.5rem', height: 'auto' }}
        data-column-id={columnId}
        disabled={isCreating}
      />
    </div>
  );
}
