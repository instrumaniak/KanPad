import { useEffect, useRef, useId } from 'react';
import DOMPurify from 'dompurify';

type MermaidApi = typeof import('mermaid')['default'];

let mermaidInitialized = false;
let mermaidPromise: Promise<MermaidApi> | null = null;
function loadMermaid(): Promise<MermaidApi> {
  if (!mermaidPromise) {
    mermaidPromise = import('mermaid').then((m) => m.default);
  }
  return mermaidPromise;
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

    loadMermaid()
      .then((mermaid) => {
        if (cancelled || seq !== renderSeqRef.current) return;
        if (!mermaidInitialized) {
          mermaid.initialize({ startOnLoad: false, theme: 'default' });
          mermaidInitialized = true;
        }
        return mermaid.render(id, code);
      })
      .then((res) => {
        if (cancelled || seq !== renderSeqRef.current) return;
        if (res?.svg && container.isConnected) {
          container.innerHTML = DOMPurify.sanitize(res.svg);
        }
      })
      .catch(() => {
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
      aria-label={`Diagram: ${code.slice(0, 50)}`}
    />
  );
}
