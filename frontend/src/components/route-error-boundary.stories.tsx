import type { Meta, StoryObj } from '@storybook/react-vite';
import { RouteErrorBoundary } from './route-error-boundary';

const meta = { title: 'Components/RouteErrorBoundary', component: RouteErrorBoundary, tags: ['autodocs'] } satisfies Meta<typeof RouteErrorBoundary>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: { children: <div>Child content rendered normally.</div> },
  render: (args) => <RouteErrorBoundary {...args} />,
};