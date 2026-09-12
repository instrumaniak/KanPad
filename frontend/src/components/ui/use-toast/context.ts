import { createContext } from 'react';

export type ToastType = 'default' | 'success' | 'error' | 'destructive';

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: string;
  title: string;
  description?: string;
  type: ToastType;
  action?: ToastAction;
  isExiting?: boolean;
}

export interface ToastContextType {
  toasts: Toast[];
  toast: (options: { title: string; description?: string; type?: ToastType; action?: ToastAction }) => void;
  dismiss: (id: string) => void;
}

export const ToastContext = createContext<ToastContextType | undefined>(undefined);
