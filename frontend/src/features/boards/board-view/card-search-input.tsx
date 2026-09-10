import { Search, X } from 'lucide-react';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { useRef } from 'react';

interface CardSearchInputProps {
  value: string;
  onChange: (v: string) => void;
  onClear: () => void;
}

export function CardSearchInput({ value, onChange, onClear }: CardSearchInputProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const handleClear = () => {
    onClear();
    inputRef.current?.focus();
  };
  return (
    <div role="search" className="relative no-native-search-cancel">
      <style>{`
        .no-native-search-cancel input[type="search"]::-webkit-search-cancel-button {
          display: none;
        }
      `}</style>
      <Search className="absolute left-2.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
      <Input
        aria-label="Search cards"
        placeholder="Search cards..."
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            handleClear();
          }
        }}
        ref={inputRef}
        className="h-9 w-[140px] pl-8 pr-11 sm:w-[240px]"
      />
      {value.length > 0 && (
        <Button
          variant="ghost"
          size="icon"
          aria-label="Clear search"
          onClick={handleClear}
          className="absolute right-1 top-1/2 min-h-[44px] min-w-[44px] -translate-y-1/2 sm:min-h-0 sm:min-w-0"
        >
          <X className="h-4 w-4" />
        </Button>
      )}
    </div>
  );
}
