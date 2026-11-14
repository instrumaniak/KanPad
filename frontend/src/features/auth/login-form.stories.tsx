import type { Meta, StoryObj } from '@storybook/react-vite';
import { expect, fn, userEvent } from 'storybook/test';
import { LoginForm } from './login-form';
import { AuthContext } from './auth-context';
import type { AuthContextType } from './auth-context';

const login = fn<AuthContextType['login']>().mockResolvedValue(undefined);

const loggedOut: AuthContextType = {
  user: null,
  isLoading: false,
  register: async () => {},
  login,
  logout: async () => {},
};

const meta = {
  title: 'Features/Auth/LoginForm',
  component: LoginForm,
  tags: ['autodocs'],
  decorators: [
    (Story) => (
      <AuthContext.Provider value={loggedOut}>
        <Story />
      </AuthContext.Provider>
    ),
  ],
} satisfies Meta<typeof LoginForm>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvas }) => {
    await userEvent.type(await canvas.findByLabelText('Email'), 'demo@example.com');
    await userEvent.type(await canvas.findByLabelText('Password'), 'secret123');
    await userEvent.click(await canvas.findByRole('button', { name: 'Sign In' }));
    await expect(login).toHaveBeenCalledWith({ email: 'demo@example.com', password: 'secret123' });
  },
};
