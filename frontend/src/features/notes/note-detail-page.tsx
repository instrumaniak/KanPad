import { useParams, useNavigate } from 'react-router-dom';
import { useNote, useDeleteNote } from './use-notes';
import { NoteDetail } from './note-detail';
import { useToast } from '@/components/ui/use-toast';
import { LoadingSkeleton } from '@/components/loading-skeleton';

export function NoteDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { toast } = useToast();
  const deleteMutation = useDeleteNote();
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
    <NoteDetail
      note={note}
      onBack={() => navigate('/notes')}
      onEdit={() => {
        navigate(`/notes/${noteId}/edit`);
      }}
      onDelete={() => {
        deleteMutation.mutate(
          { id: noteId },
          {
            onSuccess: () => {
              toast({ title: 'Note deleted' });
              navigate('/notes');
            },
            onError: () => {
              toast({ title: 'Failed to delete note', type: 'destructive' });
            },
          },
        );
      }}
    />
  );
}
