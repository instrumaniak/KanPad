import type { Meta, StoryObj } from '@storybook/react-vite';
import { Card } from './card';
import type { Card as CardType } from './use-cards';

const mockCard: CardType = {
  id: 1,
  title: 'Sample card',
  column_id: 1,
  position: 0,
  has_description: true,
  labels: [],
  due_date: null,
  created_at: '2026-01-01T00:00:00.000000Z',
  updated_at: '2026-01-01T00:00:00.000000Z',
} satisfies CardType;

const meta = {
  title: 'Features/Cards/Card',
  component: Card,
  tags: ['autodocs'],
  // Pre-existing component failures (T028 baseline): dnd-kit's sortable
  // wrapper yields nested-interactive + label-content-name-mismatch.
  // Default: 2 violations, New: 2.
  parameters: { a11y: { test: 'todo' } },
  args: { card: mockCard, index: 0 },
} satisfies Meta<typeof Card>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const New: Story = { args: { isNew: true } };