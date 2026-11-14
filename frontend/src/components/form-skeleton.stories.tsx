import type { Meta, StoryObj } from '@storybook/react-vite';
import { FormSkeleton } from './form-skeleton';

const meta = { title: 'Components/FormSkeleton', component: FormSkeleton, tags: ['autodocs'] } satisfies Meta<typeof FormSkeleton>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Default: Story = {};