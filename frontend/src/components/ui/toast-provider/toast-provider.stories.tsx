import type { Meta, StoryObj } from '@storybook/react-vite';
import { Button } from '../button';
import { ToastProvider } from './index';
import { useToast } from '../use-toast';

function Demo() {
  const { toast } = useToast();
  return (
    <Button
      onClick={() =>
        toast({ title: 'Scheduled: Catch up', description: 'Friday, February 10, 2023 at 5:57 PM' })
      }
    >
      Show toast
    </Button>
  );
}

// preview.ts already wraps every story in ToastProvider — no local decorator
// (it would double-wrap, nesting a second provider under the global one).
const meta = {
  title: 'UI/ToastProvider',
  component: ToastProvider,
  tags: ['autodocs'],
} satisfies Meta<typeof ToastProvider>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: { children: <Demo /> },
  render: () => <Demo />,
};