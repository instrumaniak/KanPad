import type { Meta, StoryObj } from '@storybook/react-vite';
import { MarkdownPreview } from './markdown-preview';

const meta = {
  title: 'Features/Cards/MarkdownPreview',
  component: MarkdownPreview,
  tags: ['autodocs'],
  args: { content: '# Hello\n\nThis is **markdown**.' },
} satisfies Meta<typeof MarkdownPreview>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Empty: Story = { args: { content: '' } };