import { useParams, useNavigate } from 'react-router-dom';
import { useNote } from './use-notes';
import { NoteEditor } from './note-editor';
import { LoadingSkeleton } from '@/components/loading-skeleton';

export function NoteEditPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const noteId = Number(id);
  const { data, isLoading, error } = useNote(noteId);

  if (isLoading) {
    return (
      <div className="p-6">
        <LoadingSkeleton count={3} />
      </div>
    );
  }

  if (error || !data?.data) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-4">
        <p className="text-muted-foreground">Note not found</p>
        <button
          onClick={() => navigate('/notes')}
          className="text-sm text-primary underline"
        >
          Back to notes
        </button>
      </div>
    );
  }

  const note = data.data;

  return (
    <div className="p-6">
      <NoteEditor
        note={note}
        onSave={() => navigate(`/notes/${noteId}`)}
        onCancel={() => navigate(`/notes/${noteId}`)}
      />
    </div>
  );
}
