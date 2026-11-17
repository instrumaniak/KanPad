import { useEffect, useRef, useId } from 'react';
import DOMPurify from 'dompurify';

type PeleLazy = typeof import('pele/lazy');

let peleLazyPromise: Promise<PeleLazy> | null = null;
function loadPele(): Promise<PeleLazy> {
  if (!peleLazyPromise) {
    // A rejected import must not poison the cache forever — retry on next render.
    peleLazyPromise = import('pele/lazy').catch((err) => {
      peleLazyPromise = null;
      throw err;
    });
  }
  return peleLazyPromise;
}

interface MermaidDiagramProps {
  code: string;
}

export function MermaidDiagram({ code }: MermaidDiagramProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const id = useId();
  const renderSeqRef = useRef(0);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    let cancelled = false;
    const seq = ++renderSeqRef.current;

    // pele ids: letters/digits/-/_ only (React useId contains ':')
    const idPrefix = id.replace(/[^a-zA-Z0-9_-]/g, '');

    loadPele()
      .then((pele) =>
        pele.renderAsync(code, {
          idPrefix,
          // Must match --pele-font / --pele-font-mono exactly (drawn = measured)
          fontFamily: 'Inter, system-ui, -apple-system, sans-serif',
          fontFamilyMono: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
          // pele default allows http/https images — a user diagram could hotlink a
          // remote image (silent third-party fetch on every note view). Off.
          images: false,
        }),
      )
      .then((res) => {
        if (cancelled || seq !== renderSeqRef.current) return;
        if (container.isConnected) {
          if (res?.svg) {
            container.innerHTML = DOMPurify.sanitize(res.svg, { FORBID_TAGS: ['style'] });
          } else {
            container.innerHTML = '<pre class="text-red-500 text-sm">Failed to render diagram</pre>';
          }
        }
      })
      .catch(() => {
        // PeleError (and any other failure) → static fallback. pele messages can
        // echo diagram source, so they are never rendered.
        if (cancelled || seq !== renderSeqRef.current) return;
        if (container.isConnected) {
          container.innerHTML = '<pre class="text-red-500 text-sm">Failed to render diagram</pre>';
        }
      });

    return () => {
      cancelled = true;
      if (container.isConnected) container.innerHTML = '';
    };
  }, [code, id]);

  return (
    <div
      ref={containerRef}
      className="my-4 flex justify-center"
      role="img"
      aria-label="Diagram"
    />
  );
}
