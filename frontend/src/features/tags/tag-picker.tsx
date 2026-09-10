import { useState } from 'react';
import { useTags, useCreateTag } from './use-tags';
import { TagBadge } from './tag-badge';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useToast } from '@/components/ui/use-toast';
import { Plus } from 'lucide-react';

const COLOR_PALETTE = ['teal', 'rose', 'amber', 'blue', 'green', 'purple'];

interface TagPickerProps {
  selectedTagIds: number[];
  onTagsChange: (ids: number[]) => void;
}

export function TagPicker({ selectedTagIds, onTagsChange }: TagPickerProps) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [newTagName, setNewTagName] = useState('');
  const [newTagColor, setNewTagColor] = useState(COLOR_PALETTE[0]);
  const { data: tagsData } = useTags();
  const createMutation = useCreateTag();
  const { toast } = useToast();

  const allTags = tagsData?.data ?? [];
  const selectedTags = allTags.filter((t) => selectedTagIds.includes(t.id));
  const filteredTags = allTags.filter(
    (t) =>
      t.name.toLowerCase().includes(search.toLowerCase()) &&
      !selectedTagIds.includes(t.id),
  );

  const toggleTag = (tagId: number) => {
    if (selectedTagIds.includes(tagId)) {
      onTagsChange(selectedTagIds.filter((id) => id !== tagId));
    } else {
      onTagsChange([...selectedTagIds, tagId]);
    }
  };

  const handleCreateTag = () => {
    if (!newTagName.trim()) return;
    createMutation.mutate(
      { name: newTagName.trim(), color: newTagColor },
      {
        onSuccess: (result) => {
          onTagsChange([...selectedTagIds, result.data.id]);
          setNewTagName('');
          setNewTagColor(COLOR_PALETTE[0]);
          setSearch('');
        },
        onError: (error) => {
          toast({ title: 'Failed to create tag', description: error.message, type: 'destructive' });
        },
      },
    );
  };

  return (
    <div className="space-y-2">
      {selectedTags.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {selectedTags.map((tag) => (
            <TagBadge
              key={tag.id}
              name={tag.name}
              color={tag.color}
              onRemove={() => toggleTag(tag.id)}
            />
          ))}
        </div>
      )}
      <div className="relative">
        <div
          className="flex items-center gap-1 rounded-md border border-input px-3 py-1.5 text-sm cursor-pointer hover:bg-accent"
          onClick={() => setOpen(!open)}
        >
          <Plus className="h-3.5 w-3.5" />
          <span className="text-muted-foreground">Add tags</span>
        </div>
        {open && (
          <div className="absolute top-full left-0 right-0 z-50 mt-1 rounded-md border border-border bg-card shadow-lg">
            <div className="p-2">
              <Input
                placeholder="Search tags..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="h-7 text-sm mb-2"
                autoFocus
              />
              <ScrollArea className="max-h-40">
                {filteredTags.length > 0 ? (
                  <div className="space-y-0.5">
                    {filteredTags.map((tag) => (
                      <button
                        key={tag.id}
                        className="flex w-full items-center gap-2 rounded-sm px-2 py-1 text-sm hover:bg-accent text-left"
                        onClick={() => toggleTag(tag.id)}
                      >
                        <div
                          className="h-2.5 w-2.5 rounded-full flex-shrink-0"
                          style={{ backgroundColor: tag.color }}
                        />
                        <span className="flex-1">{tag.name}</span>
                      </button>
                    ))}
                  </div>
                ) : search && (
                  <div className="px-2 py-1">
                    <div className="flex items-center gap-2 mb-2">
                      <Input
                        placeholder="New tag name..."
                        value={newTagName}
                        onChange={(e) => setNewTagName(e.target.value)}
                        className="h-7 text-sm flex-1"
                        onKeyDown={(e) => {
                          if (e.key === 'Enter') handleCreateTag();
                        }}
                      />
                    </div>
                    <div className="flex gap-1.5 mb-2">
                      {COLOR_PALETTE.map((color) => (
                        <button
                          key={color}
                          className={`h-5 w-5 rounded-full border-2 ${
                            newTagColor === color ? 'border-foreground' : 'border-transparent'
                          }`}
                          style={{ backgroundColor: color }}
                          onClick={() => setNewTagColor(color)}
                        />
                      ))}
                    </div>
                    <Button
                      size="sm"
                      className="w-full h-7 text-xs"
                      onClick={handleCreateTag}
                      disabled={!newTagName.trim() || createMutation.isPending}
                    >
                      <Plus className="h-3 w-3 mr-1" />
                      Create tag
                    </Button>
                  </div>
                )}
              </ScrollArea>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
