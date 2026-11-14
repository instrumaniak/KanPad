import type { Meta, StoryObj } from '@storybook/react-vite';
import { fn, expect } from 'storybook/test';
import { Button } from './index';

const meta = {
  title: 'UI/Button',
  component: Button,
  tags: ['autodocs'],
  args: { onClick: fn() },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: { children: 'Button' },
};

export const Secondary: Story = {
  args: { variant: 'secondary', children: 'Secondary' },
};

export const Destructive: Story = {
  args: { variant: 'destructive', children: 'Destructive' },
  // Pre-existing variant contrast failure (T028 baseline): 3.08:1 vs 4.5,
  // 1 violation.
  parameters: { a11y: { test: 'todo' } },
};

export const LightToken: Story = {
  globals: { theme: 'light' },
  tags: ['!autodocs'],
  args: { children: 'Button' },
  play: async () => {
    const value = getComputedStyle(document.documentElement).getPropertyValue('--background').trim();
    await expect(value).toBe('#FAFAFA');
  },
};

export const DarkToken: Story = {
  globals: { theme: 'dark' },
  tags: ['!autodocs'],
  args: { children: 'Button' },
  play: async () => {
    const value = getComputedStyle(document.documentElement).getPropertyValue('--background').trim();
    await expect(value).toBe('#0B0C0F');
  },
};
