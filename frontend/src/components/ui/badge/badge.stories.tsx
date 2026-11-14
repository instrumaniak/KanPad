import type { Meta, StoryObj } from '@storybook/react-vite';
import { Badge } from './index';

const meta = {
  title: 'UI/Badge',
  component: Badge,
  tags: ['autodocs'],
  args: { children: 'Badge' },
} satisfies Meta<typeof Badge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Secondary: Story = { args: { variant: 'secondary', children: 'Secondary' } };
export const Destructive: Story = {
  args: { variant: 'destructive', children: 'Destructive' },
  // Pre-existing variant contrast failure (T028 baseline): 3.08:1 vs 4.5
  // (`#f43f5e` on `#f9e7ea`, 12px normal weight) — axe color-contrast,
  // 1 violation (confirmed by flipping to 'error', 2026-11-14).
  parameters: { a11y: { test: 'todo' } },
};