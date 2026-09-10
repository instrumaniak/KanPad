import { useMemo } from 'react';
import Markdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeRaw from 'rehype-raw';
import rehypeSanitize from 'rehype-sanitize';
import rehypeHighlight from 'rehype-highlight';
import DOMPurify from 'dompurify';
import { MermaidDiagram } from './mermaid-diagram';

interface MarkdownRendererProps {
  content: string;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  const sanitizedContent = useMemo(() => DOMPurify.sanitize(content), [content]);

  return (
    <Markdown
      remarkPlugins={[remarkGfm]}
      rehypePlugins={[rehypeRaw, rehypeSanitize, rehypeHighlight]}
      components={{
        code({ className, children, ...props }) {
          const match = /language-(\w+)/.exec(className || '');
          if (match && match[1] === 'mermaid') {
            return <MermaidDiagram code={String(children)} />;
          }
          return <code className={className} {...props}>{children}</code>;
        },
      }}
    >
      {sanitizedContent}
    </Markdown>
  );
}
