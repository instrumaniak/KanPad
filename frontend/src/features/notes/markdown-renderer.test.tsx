import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MarkdownRenderer } from './markdown-renderer';

vi.mock('dompurify', () => ({
  default: {
    sanitize: (content: string) => content,
  },
}));

vi.mock('react-markdown', () => ({
  default: ({
    children,
    components,
  }: {
    children: string;
    components?: { code?: (props: { className?: string; children: React.ReactNode }) => React.ReactNode };
  }) => {
    const code = components?.code;
    if (code) {
      const CodeComponent = code as (props: { className?: string; children: React.ReactNode }) => React.ReactNode;
      return (
        <div data-testid="markdown">
          <div data-testid="content">{children}</div>
          <CodeComponent className="language-mermaid" children="graph TD; A-->B;" />
          <CodeComponent className="language-javascript" children="const x = 1;" />
          <CodeComponent className="" children="inline code" />
        </div>
      );
    }
    return <div data-testid="markdown">{children}</div>;
  },
}));

vi.mock('remark-gfm', () => ({ default: {} }));
vi.mock('rehype-raw', () => ({ default: {} }));
vi.mock('rehype-sanitize', () => ({ default: {} }));
vi.mock('rehype-highlight', () => ({ default: {} }));

vi.mock('./mermaid-diagram', () => ({
  MermaidDiagram: ({ code }: { code: string }) => <div data-testid="mermaid-diagram">{code}</div>,
}));

describe('MarkdownRenderer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders markdown content', () => {
    render(<MarkdownRenderer content="# Hello\nThis is **bold**" />);
    expect(screen.getByTestId('markdown')).toBeInTheDocument();
  });

  it('renders mermaid code blocks as MermaidDiagram', () => {
    render(<MarkdownRenderer content="```mermaid\ngraph TD; A-->B;\n```" />);
    expect(screen.getByTestId('mermaid-diagram')).toBeInTheDocument();
    expect(screen.getByText('graph TD; A-->B;')).toBeInTheDocument();
  });

  it('renders regular code blocks as code', () => {
    render(<MarkdownRenderer content="```javascript\nconst x = 1;\n```" />);
    expect(screen.getByTestId('markdown')).toBeInTheDocument();
  });

  it('renders inline code', () => {
    render(<MarkdownRenderer content="Inline `code` here" />);
    expect(screen.getByTestId('markdown')).toBeInTheDocument();
  });
});
