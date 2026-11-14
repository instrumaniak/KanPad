import type { Meta, StoryObj } from '@storybook/react-vite';
import { ScrollArea } from './scroll-area';

const meta = {
  title: 'UI/ScrollArea',
  component: ScrollArea,
  tags: ['autodocs'],
} satisfies Meta<typeof ScrollArea>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    // axe's scrollable-region-focusable rule requires the scrollable region to
    // be reachable by keyboard; the wrapper provides the focus stop (content
    // in the app — NoteCards, form fields — is already focusable).
    <ScrollArea className="h-40 w-48 rounded-md border p-2">
      <div tabIndex={0} className="outline-none">
        {Array.from({ length: 20 }).map((_, i) => (
          <div key={i} className="py-1 text-sm">
            Row {i + 1}
          </div>
        ))}
      </div>
    </ScrollArea>
  ),
};