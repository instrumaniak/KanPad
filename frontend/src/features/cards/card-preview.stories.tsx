import type { Meta, StoryObj } from '@storybook/react-vite';
import { CardPreview } from './card-preview';
import type { Card as CardType } from './use-cards';

const mockCard: CardType = {
  id: 1,
  title: 'Sample card with labels',
  column_id: 1,
  position: 0,
  has_description: true,
  labels: [{ id: 1, name: 'Bug', color: '#EF4444' }],
  due_date: '2026-12-01T00:00:00.000000Z',
  created_at: '2026-01-01T00:00:00.000000Z',
  updated_at: '2026-01-01T00:00:00.000000Z',
} satisfies CardType;

const meta = {
  title: 'Features/Cards/CardPreview',
  component: CardPreview,
  tags: ['autodocs'],
  args: { card: mockCard },
} satisfies Meta<typeof CardPreview>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};