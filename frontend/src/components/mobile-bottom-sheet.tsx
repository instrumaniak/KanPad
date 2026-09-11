import { useCallback, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { Sheet, SheetContent, SheetTitle } from "@/components/ui/sheet";
import { usePrefersReducedMotion } from "@/hooks/use-prefers-reduced-motion";

interface MobileBottomSheetProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: React.ReactNode;
  title?: string;
}

export function MobileBottomSheet({
  open,
  onOpenChange,
  children,
  title,
}: MobileBottomSheetProps) {
  const prefersReducedMotion = usePrefersReducedMotion();
  const dragRef = useRef<HTMLDivElement>(null);
  const pointerStartY = useRef(0);
  const [dragging, setDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState(0);

  const SWIPE_THRESHOLD = 50;

  const handlePointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (e.target !== dragRef.current) return;
      pointerStartY.current = e.clientY;
      setDragging(true);
      setDragOffset(0);
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    },
    [],
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!dragging) return;
      const delta = e.clientY - pointerStartY.current;
      if (delta > 0) {
        setDragOffset(delta);
      }
    },
    [dragging],
  );

  const handlePointerUp = useCallback(
    () => {
      if (!dragging) return;
      setDragging(false);
      if (dragOffset > SWIPE_THRESHOLD) {
        onOpenChange(false);
      }
      setDragOffset(0);
    },
    [dragging, dragOffset, onOpenChange],
  );

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side="bottom"
        className={cn(
          "w-full rounded-t-xl sm:max-w-none pb-safe",
          prefersReducedMotion && "duration-0"
        )}
        overlayClassName="sm:hidden"
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        <div className="relative">
          {/* Drag handle */}
          <div
            ref={dragRef}
            onPointerDown={handlePointerDown}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onPointerCancel={handlePointerUp}
            className={cn(
              "flex justify-center pt-3 pb-2 touch-none select-none",
              !prefersReducedMotion && "transition-transform",
            )}
            style={{
              transform: dragOffset > 0 ? `translateY(${dragOffset}px)` : undefined,
              transitionDuration: prefersReducedMotion ? "0ms" : undefined,
            }}
          >
            <div className="h-1 w-10 rounded-full bg-muted-foreground/30" />
          </div>

          {title && (
            <SheetTitle className="px-4 pb-2 text-base">{title}</SheetTitle>
          )}

          <div
            className={cn(
              "max-h-[70dvh] overflow-y-auto px-4 pb-4",
              dragging && "pointer-events-none",
            )}
          >
            {children}
          </div>
        </div>
      </SheetContent>
    </Sheet>
  );
}
