import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MarkdownPreview } from './markdown-preview';

describe('MarkdownPreview (dependency-free inline renderer)', () => {
  it('renders bold, italic and inline code', () => {
    render(<MarkdownPreview content="**Fix** the `login` *loop* bug" />);
    expect(screen.getByText('Fix').tagName).toBe('STRONG');
    expect(screen.getByText('login').tagName).toBe('CODE');
    expect(screen.getByText('loop').tagName).toBe('EM');
  });

  it('renders ++underline++ as <u>, distinct from links', () => {
    render(<MarkdownPreview content="++sign it++ and [docs](https://example.com)" />);
    const u = screen.getByText('sign it');
    expect(u.tagName).toBe('U');
    expect(u).toHaveClass('underline', 'underline-offset-2');
    // link text is NOT underlined-neutral: it carries the accent class instead
    const link = screen.getByRole('link', { name: 'docs' });
    expect(link).toHaveClass('text-label-blue');
  });

  it('leaves unpaired or inner-+ underline markers as literal text', () => {
    const first = render(<MarkdownPreview content="++no closing marker" />);
    expect(document.querySelector('u')).toBeNull();
    expect(screen.getByText(/\+\+no closing marker/)).toBeInTheDocument();
    first.unmount();
    render(<MarkdownPreview content="++a + b++" />);
    expect(document.querySelector('u')).toBeNull();
    expect(screen.getByText(/\+\+a \+ b\+\+/)).toBeInTheDocument();
  });

  it('renders http(s) links with target=_blank and rel=noopener', () => {
    render(<MarkdownPreview content="[docs](https://example.com/spec)" />);
    const link = screen.getByRole('link', { name: 'docs' });
    expect(link).toHaveAttribute('href', 'https://example.com/spec');
    expect(link).toHaveAttribute('target', '_blank');
    expect(link).toHaveAttribute('rel', expect.stringContaining('noopener'));
  });

  it('refuses javascript: links (rendered as plain text)', () => {
    render(<MarkdownPreview content="[click](javascript:alert(1))" />);
    expect(screen.queryByRole('link')).toBeNull();
    // inert literal text is fine — what matters is no javascript: URL is ever emitted
    expect(document.querySelector('a[href]')).toBeNull();
  });

  it('refuses data: and vbscript: links', () => {
    render(<MarkdownPreview content="[a](data:text/html,x) [b](vbscript:x)" />);
    expect(screen.queryByRole('link')).toBeNull();
  });

  it('never produces raw HTML — markup is escaped as text', () => {
    const { container } = render(<MarkdownPreview content="<img src=x onerror=alert(1)> broken" />);
    expect(container.querySelector('img')).toBeNull();
    expect(screen.getByText(/<img src=x onerror=alert\(1\)>/)).toBeInTheDocument();
  });

  it('renders nothing for blank content', () => {
    const { container } = render(<MarkdownPreview content="   " />);
    expect(container).toBeEmptyDOMElement();
  });

  it('leaves unmatched markdown as literal text', () => {
    render(<MarkdownPreview content="no markdown here" />);
    expect(screen.getByText('no markdown here')).toBeInTheDocument();
  });

  it('renders multiple constructs in one string', () => {
    render(<MarkdownPreview content="**a** and **b** and `c`" />);
    expect(screen.getAllByText(/^(a|b)$/).length).toBe(2);
    expect(screen.getByText('c').tagName).toBe('CODE');
  });
});
