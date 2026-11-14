import type { Meta, StoryObj } from '@storybook/react-vite';
import { useState } from 'react';
import { Calendar } from './calendar';

const meta = {
  title: 'UI/Calendar',
  component: Calendar,
  tags: ['autodocs'],
  // Pre-existing library-level failure (T028 baseline): react-day-picker's
  // ordinal aria-labels ("November 1st") don't contain the visible numeric
  // day text ("1") — axe label-content-name-mismatch on all 35 day buttons.
  parameters: { a11y: { test: 'todo' } },
} satisfies Meta<typeof Calendar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => {
    const [date, setDate] = useState<Date | undefined>(new Date());
    return <Calendar mode="single" selected={date} onSelect={setDate} />;
  },
};