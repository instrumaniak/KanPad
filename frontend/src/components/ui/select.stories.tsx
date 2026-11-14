import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './select';

const meta = {
  title: 'UI/Select',
  component: Select,
  tags: ['autodocs'],
} satisfies Meta<typeof Select>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    // Radix renders the placeholder inside the trigger, but axe's button-name
    // check ignores combobox subtrees — associate a visible <label> instead
    // (same pattern the app uses in board-form-modal / login-form).
    <div className="space-y-2">
      <label htmlFor="fruit-select" className="text-sm font-medium">
        Fruit
      </label>
      <Select>
        <SelectTrigger id="fruit-select" className="w-48">
          <SelectValue placeholder="Select a fruit" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="apple">Apple</SelectItem>
          <SelectItem value="banana">Banana</SelectItem>
          <SelectItem value="orange">Orange</SelectItem>
        </SelectContent>
      </Select>
    </div>
  ),
};