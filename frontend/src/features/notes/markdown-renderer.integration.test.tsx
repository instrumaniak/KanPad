/**
 * Integration tests for the full markdown renderer with the real unified
 * pipeline (react-markdown + remarkGfm + remarkUnderline + rehypeRaw +
 * rehypeSanitize + rehypeHighlight) — no module mocks. Verifies the
 * `++underline++` extension end-to-end, including nesting and sanitize.
 */
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MarkdownRenderer } from './markdown-renderer';

describe('MarkdownRenderer (integration, real markdown pipeline)', () => {
  it('renders ++underline++ as a <u> element', () => {
    const { container } = render(<MarkdownRenderer content="++sign it++" />);
    const u = container.querySelector('u');
    expect(u).not.toBeNull();
    expect(u?.textContent).toBe('sign it');
    expect(u).toHaveClass('underline', 'underline-offset-2');
  });

  it('supports nesting: ++**bold underline**++', () => {
    const { container } = render(<MarkdownRenderer content="++**bold underline**++" />);
    const u = container.querySelector('u');
    expect(u).not.toBeNull();
    const strong = u?.querySelector('strong');
    expect(strong?.textContent).toBe('bold underline');
  });

  it('supports emphasis inside underline: ++*quiet*++', () => {
    const { container } = render(<MarkdownRenderer content="++*quiet*++" />);
    const u = container.querySelector('u');
    expect(u?.querySelector('em')?.textContent).toBe('quiet');
  });

  it('raw <u> HTML passes the extended sanitize schema', () => {
    const { container } = render(<MarkdownRenderer content="<u>raw underline</u>" />);
    expect(container.querySelector('u')?.textContent).toBe('raw underline');
  });

  it('links render with the label-blue accent and stay target=_blank', () => {
    render(<MarkdownRenderer content="[docs](https://example.com/spec)" />);
    const link = screen.getByRole('link', { name: 'docs' });
    expect(link).toHaveClass('text-label-blue', 'underline', 'underline-offset-2');
    expect(link).toHaveAttribute('target', '_blank');
    expect(link).toHaveAttribute('rel', expect.stringContaining('noopener'));
  });

  it('script markup never renders', () => {
    const { container } = render(<MarkdownRenderer content="<script>alert(1)</script>safe" />);
    expect(container.querySelector('script')).toBeNull();
    expect(screen.getByText(/safe/)).toBeInTheDocument();
  });

  it('unbalanced ++ markers render as literal text', () => {
    const { container } = render(<MarkdownRenderer content="++never closed" />);
    expect(container.querySelector('u')).toBeNull();
    expect(screen.getByText(/\+\+never closed/)).toBeInTheDocument();
  });
});
