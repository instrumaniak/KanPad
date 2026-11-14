import type { Meta, StoryObj } from '@storybook/react-vite';
import { fn } from 'storybook/test';
import { BoardCard } from './board-card';
import type { Board } from './use-boards';

const mockBoard: Board = {
  id: 1,
  name: 'Sample board',
  background_color: '#3B82F6',
  is_archived: false,
  created_at: '2026-01-01T00:00:00.000000Z',
  updated_at: '2026-01-01T00:00:00.000000Z',
  view_mode: 'board',
  project: null,
  project_id: null,
} satisfies Board;

const meta = {
  title: 'Features/Boards/BoardCard',
  component: BoardCard,
  tags: ['autodocs'],
  args: { board: mockBoard, onEdit: fn(), onDelete: fn() },
} satisfies Meta<typeof BoardCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};