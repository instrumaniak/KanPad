import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MermaidDiagram } from './mermaid-diagram';

const mockMermaidRender = vi.fn();
const mockMermaidInitialize = vi.fn();

vi.mock('mermaid', () => ({
  default: {
    initialize: mockMermaidInitialize,
    render: mockMermaidRender,
  },
}));

describe('MermaidDiagram', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders container with role img and initializes mermaid', async () => {
    mockMermaidRender.mockResolvedValue({ svg: '<svg>diagram</svg>' });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(document.querySelector('[role="img"]')).toBeTruthy();
    });
    expect(document.querySelector('[role="img"]')).toHaveAttribute('role', 'img');
    expect(mockMermaidInitialize).toHaveBeenCalledWith({ startOnLoad: false, theme: 'default' });
  });

  it('renders SVG when mermaid succeeds', async () => {
    mockMermaidRender.mockResolvedValue({ svg: '<svg>mock-svg</svg>' });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(document.querySelector('svg')).toBeTruthy();
    });
  });

  it('shows error fallback when mermaid fails', async () => {
    mockMermaidRender.mockRejectedValue(new Error('Parse error'));
    render(<MermaidDiagram code="invalid" />);
    await waitFor(() => {
      expect(screen.getByText('Failed to render diagram')).toBeInTheDocument();
    });
  });

  it('sets aria-label from code', async () => {
    mockMermaidRender.mockResolvedValue({ svg: '<svg>diagram</svg>' });
    render(<MermaidDiagram code="graph TD; A-->B;" />);
    await waitFor(() => {
      expect(document.querySelector('[role="img"]')).toBeTruthy();
    });
    expect(document.querySelector('[role="img"]')).toHaveAttribute('aria-label', 'Diagram: graph TD; A-->B;');
  });
});
