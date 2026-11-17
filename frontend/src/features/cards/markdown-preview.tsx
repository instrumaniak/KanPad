import type { ReactNode } from 'react';

/**
 * Minimal, dependency-free inline markdown for short single-flow text
 * (the board card face title).
 *
 * Deliberately NOT unified/micromark: every board card face renders on app
 * load, so pulling the full markdown stack into the entry would defeat the
 * lazy markdown path (T016). Supported constructs — anything else renders as
 * plain text:
 *   `**bold**`  `*italic*`  `` `code` ``  `[text](https://url)`  `++underline++`
 *
 * Known limitation: constructs do not nest (`**a [b](c)**` renders the link
 * as literal text) — deliberately simple, safe, single-pass.
 *
 * Links and `<u>` are deliberately distinct: links render
 * `text-label-blue` (theme-flipping accent) + underline; `<u>` keeps the
 * text color. Both share the underline offset.
 *
 * Links are restricted to http/https/mailto. Raw HTML is NEVER produced —
 * every output node is a React element with escaped children.
 */

const TOKEN_RE =
  /(\*\*([^*]+)\*\*|\*([^*]+)\*|`([^`]+)`|\+\+([^+]+)\+\+|\[([^\]]+)\]\(([^)\s]+)\))/g;

function isSafeUrl(url: string): boolean {
  return /^(https?:|mailto:)/i.test(url);
}

function renderInline(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  let key = 0;
  for (const match of text.matchAll(TOKEN_RE)) {
    const idx = match.index ?? 0;
    if (idx > last) out.push(text.slice(last, idx));
    const [, , bold, italic, code, underline, linkText, linkHref] = match;
    if (bold !== undefined) {
      out.push(<strong key={key++}>{bold}</strong>);
    } else if (italic !== undefined) {
      out.push(<em key={key++}>{italic}</em>);
    } else if (code !== undefined) {
      out.push(
        <code key={key++} className="rounded bg-muted px-1 py-0.5 font-mono text-xs">
          {code}
        </code>,
      );
    } else if (underline !== undefined) {
      out.push(
        <u key={key++} className="underline underline-offset-2">
          {underline}
        </u>,
      );
    } else if (linkText !== undefined && linkHref !== undefined && isSafeUrl(linkHref)) {
      out.push(
        <a
          key={key++}
          href={linkHref}
          className="text-label-blue underline underline-offset-2 hover:opacity-80"
          target="_blank"
          rel="noopener noreferrer"
          onClick={(e) => e.stopPropagation()}
        >
          {linkText}
        </a>,
      );
    } else {
      out.push(match[0]);
    }
    last = idx + match[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

interface MarkdownPreviewProps {
  content: string;
}

export function MarkdownPreview({ content }: MarkdownPreviewProps) {
  const trimmed = content.trim();
  if (!trimmed) return null;
  return <span>{renderInline(trimmed)}</span>;
}
