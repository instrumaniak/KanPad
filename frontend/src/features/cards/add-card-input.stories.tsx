import type { Meta, StoryObj } from '@storybook/react-vite';
import { AddCardInput } from './add-card-input';

const meta = {
  title: 'Features/Cards/AddCardInput',
  component: AddCardInput,
  tags: ['autodocs'],
  args: { columnId: 1 },
} satisfies Meta<typeof AddCardInput>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};