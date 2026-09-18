import { useState, useEffect, useRef, useCallback, Suspense } from 'react';
import type { Note, CreateNoteData } from './notes.api';
import { useCreateNote, useUpdateNote } from './use-notes';
import { TagPicker } from '@/features/tags/tag-picker';
import { NoteLinkSelector } from './note-link-selector';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Separator } from '@/components/ui/separator';
import { useToastHelpers } from '@/lib/toast-helpers';
import { Bold, Italic, Heading, Code, List, Eye, EyeOff, Save, Workflow } from 'lucide-react';
import { LazyLoadBoundary } from '@/components/lazy-load-boundary';
import { LazyMarkdownRenderer } from './lazy-markdown-renderer';

interface NoteEditorProps {
  note?: Note;
  onSave: () => void;
  onCancel: () => void;
  onDelete?: () => void;
  defaultBoardId?: number;
  defaultProjectId?: number;
  defaultCardId?: number;
}

export function NoteEditor({
  note,
  onSave,
  onCancel,
  onDelete,
  defaultBoardId,
  defaultProjectId,
  defaultCardId,
}: NoteEditorProps) {
  const [title, setTitle] = useState(note?.title ?? '');
  const [content, setContent] = useState(note?.content ?? '');
  const [selectedTagIds, setSelectedTagIds] = useState<number[]>(
    note?.tags?.map((t) => t.id) ?? [],
  );
  const [linkBoardId, setLinkBoardId] = useState<number | undefined>(
    defaultBoardId ?? note?.board_id ?? undefined,
  );
  const [linkProjectId, setLinkProjectId] = useState<number | undefined>(
    defaultProjectId ?? note?.project_id ?? undefined,
  );
  const [linkCardId, setLinkCardId] = useState<number | undefined>(
    defaultCardId ?? note?.card_id ?? undefined,
  );
  const [preview, setPreview] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [autoSaving, setAutoSaving] = useState(false);
  const [showSaved, setShowSaved] = useState(false);
  const autoSaveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const createMutation = useCreateNote();
  const updateMutation = useUpdateNote();
  const { showSuccess, showError } = useToastHelpers();
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const insertMarkdown = useCallback(
    (before: string, after = '') => {
      const textarea = textareaRef.current;
      if (!textarea) return;
      const start = textarea.selectionStart;
      const end = textarea.selectionEnd;
      const selected = content.substring(start, end);
      const newContent =
        content.substring(0, start) + before + selected + after + content.substring(end);
      setContent(newContent);
      setDirty(true);
      setTimeout(() => {
        textarea.focus();
        const cursorPos = start + before.length;
        textarea.setSelectionRange(
          selected ? cursorPos + selected.length : cursorPos,
          selected ? cursorPos + selected.length : cursorPos,
        );
      }, 0);
    },
    [content],
  );

  const autoSave = useCallback(() => {
    if (!dirty || !note || updateMutation.isPending) return;
    clearTimeout(autoSaveTimer.current);
    autoSaveTimer.current = setTimeout(() => {
      if (!title.trim()) return;
      setAutoSaving(true);
      updateMutation.mutate(
        {
          id: note.id,
          data: {
            title: title.trim(),
            content,
            tagIds: selectedTagIds,
            board_id: linkBoardId ?? null,
            project_id: linkProjectId ?? null,
            card_id: linkCardId ?? null,
          },
        },
        {
          onSettled: () => {
            setAutoSaving(false);
          },
          onSuccess: () => {
            setShowSaved(true);
            setTimeout(() => setShowSaved(false), 2000);
          },
          onError: () => {
            showError('Failed to auto-save');
          },
        },
      );
      setDirty(false);
    }, 2000);
  }, [
    dirty,
    note,
    title,
    content,
    selectedTagIds,
    linkBoardId,
    linkProjectId,
    linkCardId,
    updateMutation,
    showError,
  ]);

  useEffect(() => {
    if (note) {
      autoSave();
    }
    return () => clearTimeout(autoSaveTimer.current);
  }, [title, content, note, autoSave]);

  useEffect(() => {
    const handler = (e: BeforeUnloadEvent) => {
      if (dirty) {
        e.preventDefault();
      }
    };
    window.addEventListener('beforeunload', handler);
    return () => window.removeEventListener('beforeunload', handler);
  }, [dirty]);

  const handleSave = () => {
    if (!title.trim()) {
      showError('Title is required');
      return;
    }
    clearTimeout(autoSaveTimer.current);

    const commonData: CreateNoteData = {
      title: title.trim(),
      content,
      tagIds: selectedTagIds,
      board_id: linkBoardId ?? null,
      project_id: linkProjectId ?? null,
      card_id: linkCardId ?? null,
    };

    if (note) {
      updateMutation.mutate(
        { id: note.id, data: commonData },
        {
          onSuccess: () => {
            showSuccess('Note saved');
            setDirty(false);
            onSave();
          },
          onError: (error) => {
            showError('Failed to save note', error.message);
          },
        },
      );
    } else {
      createMutation.mutate(commonData, {
        onSuccess: () => {
          showSuccess('Note created');
          setDirty(false);
          onSave();
        },
        onError: (error) => {
          showError('Failed to create note', error.message);
        },
      });
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 's') {
      e.preventDefault();
      handleSave();
    }
    if (e.key === 'Escape') {
      if (dirty) {
        const confirmed = confirm('Unsaved changes will be lost.');
        if (!confirmed) return;
      }
      onCancel();
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold">{note ? 'Edit Note' : 'New Note'}</h3>
        <div className="flex items-center gap-2">
          {autoSaving && (
            <span className="flex items-center gap-1 text-xs text-muted-foreground">
              <Save className="h-3 w-3 animate-pulse" />
              Saving...
            </span>
          )}
          {showSaved && (
            <span className="flex items-center gap-1 text-xs text-green-500">
              <Save className="h-3 w-3" />
              Saved
            </span>
          )}
          <Button className="mr-8" variant="ghost" size="sm" onClick={() => setPreview(!preview)}>
            {preview ? (
              <EyeOff className="h-3.5 w-3.5 mr-1" />
            ) : (
              <Eye className="h-3.5 w-3.5 mr-1" />
            )}
            {preview ? 'Edit' : 'Preview'}
          </Button>
        </div>
      </div>

      <Input
        placeholder="Note title..."
        value={title}
        onChange={(e) => {
          setTitle(e.target.value);
          setDirty(true);
        }}
        onKeyDown={handleKeyDown}
        className="text-lg font-medium"
      />

      {!preview ? (
        <div className="space-y-2">
          <div className="flex items-center gap-1 sticky top-0 bg-background z-10 pb-1 border-b">
            <ToolbarButton
              icon={<Bold className="h-3.5 w-3.5" />}
              label="Bold"
              onClick={() => insertMarkdown('**', '**')}
            />
            <ToolbarButton
              icon={<Italic className="h-3.5 w-3.5" />}
              label="Italic"
              onClick={() => insertMarkdown('_', '_')}
            />
            <ToolbarButton
              icon={<Heading className="h-3.5 w-3.5" />}
              label="Heading"
              onClick={() => insertMarkdown('### ', '')}
            />
            <ToolbarButton
              icon={<Code className="h-3.5 w-3.5" />}
              label="Code"
              onClick={() => insertMarkdown('```\n', '\n```')}
            />
            <ToolbarButton
              icon={<List className="h-3.5 w-3.5" />}
              label="List"
              onClick={() => insertMarkdown('- ', '')}
            />
            <ToolbarButton
              icon={<Workflow className="h-3.5 w-3.5" />}
              label="Mermaid"
              onClick={() => insertMarkdown('```mermaid\n', '\n```')}
            />
          </div>
          <textarea
            ref={textareaRef}
            placeholder="Write your note content in markdown..."
            value={content}
            onChange={(e) => {
              setContent(e.target.value);
              setDirty(true);
            }}
            onKeyDown={handleKeyDown}
            className="w-full min-h-[300px] rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring resize-y font-mono"
          />
        </div>
      ) : (
        <div className="min-h-[300px] rounded-md border border-input bg-card px-3 py-2 text-sm overflow-auto prose prose-sm dark:prose-invert max-w-none">
          <LazyLoadBoundary fallback={<div className="text-xs text-muted-foreground">Failed to load preview</div>}>
            <Suspense fallback={<div className="text-xs text-muted-foreground animate-pulse">Loading...</div>}>
              <LazyMarkdownRenderer content={content} />
            </Suspense>
          </LazyLoadBoundary>
        </div>
      )}

      <NoteLinkSelector
        linkBoardId={linkBoardId}
        linkProjectId={linkProjectId}
        linkCardId={linkCardId}
        onLinkBoard={(id) => {
          setLinkBoardId(id);
          setDirty(true);
        }}
        onLinkProject={(id) => {
          setLinkProjectId(id);
          setDirty(true);
        }}
        onLinkCard={(id) => {
          setLinkCardId(id);
          setDirty(true);
        }}
      />

      <Separator />

      <div>
        <label className="text-sm font-medium mb-1 block">Tags</label>
        <TagPicker selectedTagIds={selectedTagIds} onTagsChange={setSelectedTagIds} />
      </div>

      <div className="flex items-center justify-end gap-2 pt-2 border-t">
        <Button variant="outline" size="sm" onClick={onCancel}>
          Cancel
        </Button>

        {note && onDelete && (
          <Button variant="destructive" size="sm" onClick={onDelete}>
            Delete
          </Button>
        )}
        <Button
          size="sm"
          onClick={handleSave}
          disabled={createMutation.isPending || updateMutation.isPending}
        >
          {createMutation.isPending || updateMutation.isPending ? 'Saving...' : 'Save'}
        </Button>
      </div>
    </div>
  );
}

function ToolbarButton({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button variant="ghost" size="sm" className="h-7 w-7 p-0" onClick={onClick} title={label}>
      {icon}
      <span className="sr-only">{label}</span>
    </Button>
  );
}
