import type { Meta, StoryObj } from '@storybook/react-vite';
import { Checkbox } from './checkbox';

const meta = {
  title: 'UI/Checkbox',
  component: Checkbox,
  tags: ['autodocs'],
  args: { 'aria-label': 'Accept terms' },
} satisfies Meta<typeof Checkbox>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
export const Checked: Story = { args: { checked: true, 'aria-label': 'Checked' } };