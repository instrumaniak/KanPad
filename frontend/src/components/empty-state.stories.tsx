import type { Meta, StoryObj } from '@storybook/react-vite';
import { EmptyState } from './empty-state';

const meta = {
  title: 'Components/EmptyState',
  component: EmptyState,
  tags: ['autodocs'],
  args: { icon: '📭', headline: 'No items yet', description: 'Create your first item to get started.' },
} satisfies Meta<typeof EmptyState>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};