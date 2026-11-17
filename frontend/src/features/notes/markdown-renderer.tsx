import { useMemo } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize';
import rehypeHighlight from 'rehype-highlight';
import DOMPurify from 'dompurify';
import { MermaidDiagram } from './mermaid-diagram';
import { remarkUnderline } from './remark-underline';

// Allow <u> — the only hand-added tag on top of the GitHub-style default
// schema (emitted by remarkUnderline via data.hName, and by users writing
// raw HTML). Nothing else widens.
const sanitizeSchema = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames ?? []), 'u'],
};

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  const sanitizedContent = useMemo(() => DOMPurify.sanitize(content), [content]);

  return (
    <Markdown
      remarkPlugins={[remarkGfm, remarkUnderline]}
      rehypePlugins={[rehypeRaw, [rehypeSanitize, sanitizeSchema], rehypeHighlight]}
      components={{
        h1: ({ children }) => <h1 className="text-lg font-bold">{children}</h1>,
        h2: ({ children }) => <h2 className="text-base font-semibold">{children}</h2>,
        p: ({ children }) => <p className="leading-relaxed">{children}</p>,
        ul: ({ children }) => <ul className="list-disc pl-5">{children}</ul>,
        ol: ({ children }) => <ol className="list-decimal pl-5">{children}</ol>,
        table: ({ children }) => <table className="my-2 border-collapse text-sm">{children}</table>,
        th: ({ children }) => (
          <th className="border border-border px-2 py-1 text-left font-semibold">{children}</th>
        ),
        td: ({ children }) => <td className="border border-border px-2 py-1">{children}</td>,
        code({ className, children }) {
          const match = /language-(\w+)/.exec(className || '');
          if (match && match[1] === 'mermaid') {
            return <MermaidDiagram code={String(children)} />;
          }
          // Fenced blocks carry a language-*/hljs class — render them plain
          // (the <pre> wrapper owns box styling); chip style is inline-only.
          // hljs adds classes only for known languages, so also treat
          // multi-line code as block even without a language class.
          const text = String(children);
          const isBlock = !!className || text.includes('\n');
          if (isBlock) {
            return <code className={className}>{children}</code>;
          }
          return (
            <code className="rounded bg-muted px-1 py-0.5 font-mono text-xs">
              {children}
            </code>
          );
        },
        pre: ({ children }) => <pre className="rounded bg-muted p-2 overflow-auto">{children}</pre>,
        u: ({ children }) => <u className="underline underline-offset-2">{children}</u>,
        a: ({ children, href }) => (
          <a href={href} className="text-label-blue underline underline-offset-2 hover:opacity-80" target="_blank" rel="noopener noreferrer">
            {children}
          </a>
        ),
      }}
    >
      {sanitizedContent}
    </Markdown>
  );
}
