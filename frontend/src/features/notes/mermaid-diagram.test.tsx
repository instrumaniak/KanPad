import { describe, it, expect, vi, beforeEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { render, screen, waitFor, act } from '@testing-library/react';
import { MermaidDiagram } from './mermaid-diagram';
import { PeleError } from 'pele/lazy';

const mockRenderAsync = vi.fn();

vi.mock('pele/lazy', async (importOriginal) => {
  const actual = await importOriginal<typeof import('pele/lazy')>();
  return {
    ...actual,
    renderAsync: (...args: unknown[]) => mockRenderAsync(...args),
  };
});

describe('MermaidDiagram', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders container with role img', async () => {
    mockRenderAsync.mockResolvedValue({ svg: '<svg>diagram</svg>', type: 'flowchart', mermaidType: 'flowchart', width: 10, height: 10, links: [] });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(document.querySelector('[role="img"]')).toBeTruthy();
    });
    expect(document.querySelector('[role="img"]')).toHaveAttribute('role', 'img');
  });

  it('renders SVG when pele succeeds', async () => {
    mockRenderAsync.mockResolvedValue({ svg: '<svg>mock-svg</svg>', type: 'flowchart', mermaidType: 'flowchart', width: 10, height: 10, links: [] });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(document.querySelector('svg')).toBeTruthy();
    });
  });

  it('passes a sanitized idPrefix (letters/digits/-/_ only)', async () => {
    mockRenderAsync.mockResolvedValue({ svg: '<svg>diagram</svg>', type: 'flowchart', mermaidType: 'flowchart', width: 10, height: 10, links: [] });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(mockRenderAsync).toHaveBeenCalled();
    });
    const options = mockRenderAsync.mock.calls[0][1] as { idPrefix: string };
    expect(options.idPrefix).toMatch(/^[a-zA-Z0-9_-]+$/);
  });

  it('passes images: false and the exact font strings', async () => {
    mockRenderAsync.mockResolvedValue({ svg: '<svg>diagram</svg>', type: 'flowchart', mermaidType: 'flowchart', width: 10, height: 10, links: [] });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(mockRenderAsync).toHaveBeenCalled();
    });
    const options = mockRenderAsync.mock.calls[0][1] as {
      images: boolean;
      fontFamily: string;
      fontFamilyMono: string;
    };
    expect(options.images).toBe(false);
    expect(options.fontFamily).toBe('Inter, system-ui, -apple-system, sans-serif');
    expect(options.fontFamilyMono).toBe('ui-monospace, SFMono-Regular, Menlo, Consolas, monospace');
  });

  it('strips unsafe markup from the pele SVG output (DOMPurify)', async () => {
    mockRenderAsync.mockResolvedValue({
      svg: '<svg><script>alert(1)</script><circle onload="alert(2)" /><text>ok</text></svg>',
      type: 'flowchart',
      mermaidType: 'flowchart',
      width: 10,
      height: 10,
      links: [],
    });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(document.querySelector('svg')).toBeTruthy();
    });
    expect(document.querySelector('script')).toBeNull();
    expect(document.querySelector('circle')?.getAttribute('onload')).toBeNull();
    expect(document.querySelector('text')?.textContent).toBe('ok');
  });

  it('strips <style> tags from the pele SVG output (defense in depth)', async () => {
    mockRenderAsync.mockResolvedValue({
      svg: '<svg><style>@import url(https://evil.example/x.css)</style><text>ok</text></svg>',
      type: 'flowchart',
      mermaidType: 'flowchart',
      width: 10,
      height: 10,
      links: [],
    });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(document.querySelector('svg')).toBeTruthy();
    });
    expect(document.querySelector('style')).toBeNull();
    expect(document.querySelector('text')?.textContent).toBe('ok');
  });

  it('shows the static fallback when pele resolves with an empty svg', async () => {
    mockRenderAsync.mockResolvedValue({
      svg: '',
      type: 'flowchart',
      mermaidType: 'flowchart',
      width: 0,
      height: 0,
      links: [],
    });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(screen.getByText('Failed to render diagram')).toBeInTheDocument();
    });
  });

  it('an earlier render resolving after a later one must not overwrite it', async () => {
    const svgFor = (label: string) => `<svg><text>${label}</text></svg>`;
    let resolveA: (v: unknown) => void = () => {};
    let resolveB: (v: unknown) => void = () => {};
    mockRenderAsync
      .mockImplementationOnce(
        () =>
          new Promise((r) => {
            resolveA = r;
          }),
      )
      .mockImplementationOnce(
        () =>
          new Promise((r) => {
            resolveB = r;
          }),
      );
    const { rerender } = render(<MermaidDiagram code="graph TD; A-->B;" />);
    rerender(<MermaidDiagram code="pie title T pie T : 1" />);
    // Wait until BOTH renders actually invoked renderAsync (the lazy import
    // resolves on a later microtask), then resolve the NEWER one first.
    await waitFor(() => {
      expect(mockRenderAsync).toHaveBeenCalledTimes(2);
    });
    resolveB({ svg: svgFor('B'), type: 'pie', mermaidType: 'pie', width: 10, height: 10, links: [] });
    await waitFor(() => {
      expect(document.querySelector('text')?.textContent).toBe('B');
    });
    resolveA({ svg: svgFor('A'), type: 'flowchart', mermaidType: 'flowchart', width: 10, height: 10, links: [] });
    // Barrier: flush A's whole resolution chain inside act() so a stale-write
    // regression is actually observable.
    await act(async () => {});
    expect(document.querySelector('text')?.textContent).toBe('B');
  });

  it('unmount mid-flight: the late resolution must not write to the DOM', async () => {
    let resolve: (v: unknown) => void = () => {};
    mockRenderAsync.mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const { unmount } = render(<MermaidDiagram code="graph TD; A-->B;" />);
    unmount();
    resolve({ svg: '<svg><text>late</text></svg>', type: 'flowchart', mermaidType: 'flowchart', width: 10, height: 10, links: [] });
    // Let the promise chain flush; nothing may land in the document.
    await new Promise((r) => setTimeout(r, 50));
    expect(document.body.innerHTML).not.toContain('late');
  });

  it('shows error fallback when pele throws PeleError', async () => {
    mockRenderAsync.mockRejectedValue(new PeleError('syntax error at line 1', 'syntax'));
    render(<MermaidDiagram code="invalid" />);
    await waitFor(() => {
      expect(screen.getByText('Failed to render diagram')).toBeInTheDocument();
    });
    // pele error message must not leak into the DOM
    expect(screen.queryByText(/syntax error/)).toBeNull();
  });

  it('shows error fallback when pele throws a non-PeleError', async () => {
    mockRenderAsync.mockRejectedValue(new Error('unexpected boom'));
    render(<MermaidDiagram code="invalid" />);
    await waitFor(() => {
      expect(screen.getByText('Failed to render diagram')).toBeInTheDocument();
    });
    expect(screen.queryByText(/unexpected boom/)).toBeNull();
  });

  it('sets a static aria-label (does not echo diagram source)', async () => {
    mockRenderAsync.mockResolvedValue({ svg: '<svg>diagram</svg>', type: 'flowchart', mermaidType: 'flowchart', width: 10, height: 10, links: [] });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(document.querySelector('[role="img"]')).toBeTruthy();
    });
    expect(document.querySelector('[role="img"]')).toHaveAttribute('aria-label', 'Diagram');
  });

  it('keeps the drawn font === measured font invariant (--pele-font(-mono) matches the JS render options)', () => {
    const css = readFileSync(join(process.cwd(), 'src/index.css'), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
    const component = readFileSync(join(process.cwd(), 'src/features/notes/mermaid-diagram.tsx'), 'utf8');
    const normalize = (s: string) => s.replace(/\s+/g, ' ').trim().toLowerCase();
    for (const [cssVar, jsKey] of [
      ['--pele-font:', 'fontFamily:'],
      ['--pele-font-mono:', 'fontFamilyMono:'],
    ] as const) {
      const cssMatch = css.match(new RegExp(`${cssVar.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*([^;]+);`));
      const jsMatch = component.match(new RegExp(`${jsKey.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*'([^']+)'`));
      expect(cssMatch, `missing ${cssVar} in index.css`).toBeTruthy();
      expect(jsMatch, `missing ${jsKey} in mermaid-diagram.tsx`).toBeTruthy();
      expect(normalize(cssMatch![1])).toBe(normalize(jsMatch![1]));
    }
  });
});
