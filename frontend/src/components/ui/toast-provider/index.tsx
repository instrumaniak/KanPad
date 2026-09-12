import * as React from 'react';
import { ToastContext, type Toast, type ToastAction, type ToastType } from '../use-toast/context';
import './toast-provider.css';

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = React.useState<Toast[]>([]);
  const timeoutRefs = React.useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  React.useEffect(() => {
    const refs = timeoutRefs.current;
    return () => {
      refs.forEach((id) => clearTimeout(id));
      refs.clear();
    };
  }, []);

  const toast = React.useCallback(
    (options: { title: string; description?: string; type?: ToastType; action?: ToastAction }) => {
      const id = crypto.randomUUID();
      const type = options.type || 'default';
      setToasts((prev) => [...prev, { ...options, id, type, isExiting: false }]);

      const duration = type === 'destructive' && options.action ? 5000 : 3000;
      const timeoutId = setTimeout(() => {
        timeoutRefs.current.delete(id);
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, duration);
      timeoutRefs.current.set(id, timeoutId);
    },
    [],
  );

  const dismiss = React.useCallback((id: string) => {
    const existing = timeoutRefs.current.get(id);
    if (existing) {
      clearTimeout(existing);
      timeoutRefs.current.delete(id);
    }
    setToasts((prev) =>
      prev.map((t) => (t.id === id ? { ...t, isExiting: true } : t))
    );
    const exitTimeoutId = setTimeout(() => {
      timeoutRefs.current.delete(`${id}-exit`);
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 200);
    timeoutRefs.current.set(`${id}-exit`, exitTimeoutId);
  }, []);

  return (
    <ToastContext.Provider value={{ toasts, toast, dismiss }}>
      {children}
      <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2" data-testid="toast-container">
        {toasts.map((t) => (
          <div
            key={t.id}
            data-testid={`toast-${t.type}`}
            role={t.type === 'destructive' || t.type === 'error' ? 'alert' : 'status'}
            aria-live={t.type === 'destructive' || t.type === 'error' ? 'assertive' : 'polite'}
            className={`toast-enter ${t.isExiting ? 'toast-exit' : ''} rounded-lg border p-4 shadow-lg transition-all ${
              t.type === 'destructive' || t.type === 'error'
                ? 'border-destructive bg-destructive/10 text-destructive'
                : t.type === 'success'
                  ? 'border-[var(--success)] bg-[var(--success)]/10 text-[var(--success)]'
                  : 'border-border bg-background text-foreground'
            }`}
          >
            <div className="flex items-start justify-between gap-2">
              <div>
                <p className="text-sm font-medium">{t.title}</p>
                {t.description && <p className="text-sm opacity-80">{t.description}</p>}
              </div>
              <div className="flex items-center gap-2">
                {t.action && (
                  <button
                    onClick={() => {
                      try {
                        t.action!.onClick();
                      } catch (e) {
                        console.error('Toast action failed:', e);
                      }
                      dismiss(t.id);
                    }}
                    className="text-sm font-medium underline hover:opacity-80"
                  >
                    {t.action.label}
                  </button>
                )}
                <button
                  onClick={() => dismiss(t.id)}
                  aria-label="Dismiss notification"
                  className="text-muted-foreground hover:text-foreground"
                >
                  ×
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
