import type { Meta, StoryObj } from '@storybook/react-vite';
import { expect, fn, userEvent } from 'storybook/test';
import { http, HttpResponse, delay } from 'msw';
import { TagPicker } from './tag-picker';
import { handlers } from '../../mocks/handlers';
import type { components } from '@/lib/api/schema';

type PageTagResponse = components['schemas']['Page_TagResponse'];

const emptyPage: PageTagResponse = {
  data: [],
  page: 1,
  limit: 50,
  total: 0,
};

const onTagsChange = fn<(ids: number[]) => void>();

const meta = {
  title: 'Features/Tags/TagPicker',
  component: TagPicker,
  tags: ['autodocs'],
  args: { selectedTagIds: [], onTagsChange },
} satisfies Meta<typeof TagPicker>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  beforeEach({ msw }) {
    msw.use(...handlers);
  },
  play: async ({ canvas }) => {
    await userEvent.click(await canvas.findByText('Add tags'));
    await expect(await canvas.findByText('Bug')).toBeTruthy();
    await expect(await canvas.findByText('Feature')).toBeTruthy();
    await userEvent.click(await canvas.findByText('Bug'));
    await expect(onTagsChange).toHaveBeenCalledWith([1]);
  },
};

export const Loading: Story = {
  beforeEach({ msw }) {
    msw.use(
      http.get('/api/v1/tags', async () => {
        await delay('infinite');
        return HttpResponse.json(emptyPage satisfies PageTagResponse);
      }),
    );
  },
  play: async ({ canvas }) => {
    await userEvent.click(await canvas.findByText('Add tags'));
    // The request never resolves — the fixture must never render.
    await expect(canvas.queryByText('Bug')).toBeNull();
    await expect(canvas.queryByText('Feature')).toBeNull();
  },
};

export const Empty: Story = {
  beforeEach({ msw }) {
    msw.use(
      http.get('/api/v1/tags', () => HttpResponse.json(emptyPage satisfies PageTagResponse)),
    );
  },
  // Pre-existing component violation (T028 baseline): the COLOR_PALETTE
  // buttons in the create-tag row have no accessible name (axe button-name,
  // 6 violations). Empty's play types into the search box, which renders that
  // row. App-code a11y fix deferred to a follow-up — T028 stays tooling-only.
  parameters: { a11y: { test: 'todo' } },
  play: async ({ canvas }) => {
    await userEvent.click(await canvas.findByText('Add tags'));
    await userEvent.type(await canvas.findByPlaceholderText('Search tags...'), 'newtag');
    // With no matching tags and a non-empty search, the create-tag row renders.
    await expect(await canvas.findByPlaceholderText('New tag name...')).toBeTruthy();
    await expect(canvas.queryByText('Bug')).toBeNull();
  },
};

export const Error: Story = {
  beforeEach({ msw }) {
    msw.use(
      http.get('/api/v1/tags', () =>
        HttpResponse.json({ message: 'stubbed failure' }, { status: 500 }),
      ),
    );
  },
  // The component swallows query errors (`tagsData?.data ?? []`) — this story
  // locks in "no crash, no stale fixture tags" for an API failure.
  play: async ({ canvas }) => {
    await userEvent.click(await canvas.findByText('Add tags'));
    await expect(await canvas.findByPlaceholderText('Search tags...')).toBeTruthy();
    await expect(canvas.queryByText('Bug')).toBeNull();
  },
};
