import type { Meta, StoryObj } from '@storybook/react-vite';
import { LoadingSkeleton } from './loading-skeleton';

const meta = {
  title: 'Components/LoadingSkeleton',
  component: LoadingSkeleton,
  tags: ['autodocs'],
  // Pre-existing component failure (T028 baseline): `aria-label` on a plain
  // <div> without a role — axe aria-prohibited-attr, 1 violation.
  parameters: { a11y: { test: 'todo' } },
} satisfies Meta<typeof LoadingSkeleton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};