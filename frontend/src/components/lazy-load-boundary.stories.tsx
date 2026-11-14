import type { Meta, StoryObj } from '@storybook/react-vite';
import { LazyLoadBoundary } from './lazy-load-boundary';

const meta = { title: 'Components/LazyLoadBoundary', component: LazyLoadBoundary, tags: ['autodocs'] } satisfies Meta<typeof LazyLoadBoundary>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: { children: <div>Loaded content</div>, fallback: <div>Loading...</div> },
  render: (args) => <LazyLoadBoundary {...args} />,
};