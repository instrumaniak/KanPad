import type { Meta, StoryObj } from '@storybook/react-vite';
import { BoardListSkeleton } from './board-list-skeleton';

const meta = { title: 'Components/BoardListSkeleton', component: BoardListSkeleton, tags: ['autodocs'] } satisfies Meta<typeof BoardListSkeleton>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Default: Story = {};