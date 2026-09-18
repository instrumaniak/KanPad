import { Search, X } from 'lucide-react';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { useRef, useId } from 'react';
import { cn } from '@/lib/utils';

interface CardSearchInputProps {
  value: string;
  onChange: (v: string) => void;
  onClear: () => void;
}

export function CardSearchInput({ value, onChange, onClear }: CardSearchInputProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const searchInputId = useId();
  const handleClear = () => {
    onClear();
    inputRef.current?.focus();
  };
  return (
    <div
      role="search"
      className={cn(
        'no-native-search-cancel group relative inline-flex items-center overflow-hidden rounded-full',
        'border border-transparent bg-muted/50',
        'transition-[inline-size,border-color,box-shadow] duration-300',
        'ease-[cubic-bezier(0.2,0.9,0.25,1)]',
        'focus-within:min-w-[360px] focus-within:border-border focus-within:shadow-sm',
        '[&:has(input:not(:placeholder-shown))]:min-w-[360px]',
        'motion-reduce:transition-none',
      )}
      style={{ inlineSize: '36px' }}
    >
      <style>{`
        .no-native-search-cancel input[type="search"]::-webkit-search-cancel-button {
          display: none;
        }
      `}</style>
      <label
        htmlFor={searchInputId}
        className="flex h-9 w-9 shrink-0 cursor-pointer items-center justify-center"
      >
        <Search className="h-4 w-4 text-muted-foreground" />
        <span className="sr-only">Search cards</span>
      </label>
      <Input
        id={searchInputId}
        aria-label="Search cards"
        placeholder="Search cards..."
        type="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            onClear();
            inputRef.current?.blur();
          }
        }}
        ref={inputRef}
        className="h-9 min-w-0 flex-1 rounded-none border-0 bg-transparent pl-0 pr-8 text-base shadow-none opacity-0 placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground transition-opacity duration-200 delay-[120ms] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-transparent group-focus-within:opacity-100 md:text-sm"
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
