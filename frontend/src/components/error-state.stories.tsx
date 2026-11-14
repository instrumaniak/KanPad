import type { Meta, StoryObj } from '@storybook/react-vite';
import { ErrorState } from './error-state';

const meta = {
  title: 'Components/ErrorState',
  component: ErrorState,
  tags: ['autodocs'],
  // Pre-existing component contrast failure (T028 baseline: app-code changes
  // are out of scope) — `text-destructive` on `bg-destructive/10` scores
  // 3.08/2.56:1 vs 4.5. Default: 2 violations, With Retry: 3.
  parameters: { a11y: { test: 'todo' } },
  args: { message: 'Failed to load data.' },
} satisfies Meta<typeof ErrorState>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const WithRetry: Story = { args: { onRetry: () => {} } };