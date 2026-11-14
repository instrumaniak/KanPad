import type { Meta, StoryObj } from '@storybook/react-vite';
import { expect, fn, userEvent } from 'storybook/test';
import { TagBadge } from './tag-badge';

const onRemove = fn();

const meta = {
  title: 'Features/Tags/TagBadge',
  component: TagBadge,
  tags: ['autodocs'],
  // Backend default tag color (`DEFAULT_TAG_COLOR = 'teal'`); named colors
  // keep `backgroundColor: ${color}20` invalid → transparent over canvas,
  // so the fixture uses the same values the app actually produces.
  args: { name: 'Bug', color: 'teal' },
} satisfies Meta<typeof TagBadge>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const Removable: Story = {
  args: { onRemove },
  play: async ({ canvas }) => {
    await userEvent.click(await canvas.findByRole('button', { name: /remove bug/i }));
    await expect(onRemove).toHaveBeenCalledOnce();
  },
};
