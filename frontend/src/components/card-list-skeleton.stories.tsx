import type { Meta, StoryObj } from '@storybook/react-vite';
import { CardListSkeleton } from './card-list-skeleton';

const meta = { title: 'Components/CardListSkeleton', component: CardListSkeleton, tags: ['autodocs'] } satisfies Meta<typeof CardListSkeleton>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Default: Story = {};