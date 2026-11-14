import type { Meta, StoryObj } from '@storybook/react-vite';
import { expect, fn, userEvent } from 'storybook/test';
import { RegisterForm } from './register-form';
import { AuthContext } from './auth-context';
import type { AuthContextType } from './auth-context';

const register = fn<AuthContextType['register']>().mockResolvedValue(undefined);

const loggedOut: AuthContextType = {
  user: null,
  isLoading: false,
  register,
  login: async () => {},
  logout: async () => {},
};

const meta = {
  title: 'Features/Auth/RegisterForm',
  component: RegisterForm,
  tags: ['autodocs'],
  decorators: [
    (Story) => (
      <AuthContext.Provider value={loggedOut}>
        <Story />
      </AuthContext.Provider>
    ),
  ],
} satisfies Meta<typeof RegisterForm>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  play: async ({ canvas }) => {
    await userEvent.type(await canvas.findByLabelText('Email'), 'demo@example.com');
    await userEvent.type(await canvas.findByLabelText('Password'), 'secret123');
    await userEvent.type(await canvas.findByLabelText('Confirm Password'), 'secret123');
    await userEvent.click(await canvas.findByRole('button', { name: 'Register' }));
    await expect(register).toHaveBeenCalledWith({
      email: 'demo@example.com',
      password: 'secret123',
    });
  },
};
