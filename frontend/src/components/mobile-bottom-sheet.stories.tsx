import type { Meta, StoryObj } from '@storybook/react-vite';
import { expect, screen, userEvent, waitFor } from 'storybook/test';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { MobileBottomSheet } from './mobile-bottom-sheet';

const meta = { title: 'Components/MobileBottomSheet', component: MobileBottomSheet, tags: ['autodocs'] } satisfies Meta<typeof MobileBottomSheet>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: { open: false, onOpenChange: () => {}, children: <p>Content</p>, title: 'Sheet title' },
  render: () => {
    const [open, setOpen] = useState(false);
    return (
      <>
        <Button onClick={() => setOpen(true)}>Open sheet</Button>
        <MobileBottomSheet open={open} onOpenChange={setOpen} title="Sheet title">
          <p>Content</p>
        </MobileBottomSheet>
      </>
    );
  },
  play: async () => {
    // The sheet content portals to document.body (Radix Dialog) — use `screen`, not `canvas`.
    await userEvent.click(await screen.findByRole('button', { name: 'Open sheet' }));
    await expect(await screen.findByText('Sheet title')).toBeTruthy();
    await expect(await screen.findByText('Content')).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByText('Sheet title')).toBeNull());
  },
};
