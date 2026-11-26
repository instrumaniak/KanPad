import { test as base, type Page } from '@playwright/test';

export interface ConsoleMessage {
  type: 'error' | 'warning' | 'log' | 'info' | 'debug';
  text: string;
  location: string;
}

export interface NetworkRequest {
  url: string;
  method: string;
  status: number;
  failure?: string;
}

export interface TestContext {
  consoleMessages: ConsoleMessage[];
  networkRequests: NetworkRequest[];
  jsErrors: string[];
}

export function createMonitoringFixture() {
  return base.extend<{ monitoring: TestContext }>({
    monitoring: function useMonitoring({ page }, useFixture) {
      const context: TestContext = {
        consoleMessages: [],
        networkRequests: [],
        jsErrors: [],
      };

      page.on('console', (msg) => {
        context.consoleMessages.push({
          type: msg.type() as ConsoleMessage['type'],
          text: msg.text(),
          location: msg.location()?.url || 'unknown',
        });
      });

      page.on('pageerror', (error) => {
        context.jsErrors.push(error.message);
      });

      page.on('response', (response) => {
        context.networkRequests.push({
          url: response.url(),
          method: response.request().method(),
          status: response.status(),
          failure: response.request().failure()?.errorText,
        });
      });

      return useFixture(context).then(() => {
        const errors = context.consoleMessages.filter(m => m.type === 'error');
        const jsErrors = context.jsErrors;
        const failedRequests = context.networkRequests.filter(r => r.status >= 400 || r.failure);

        if (errors.length > 0) {
          console.error('\n Browser Console Errors:');
          errors.forEach(e => console.error(`  [${e.type}] ${e.text} at ${e.location}`));
        }

        if (jsErrors.length > 0) {
          console.error('\n JavaScript Errors:');
          jsErrors.forEach(e => console.error(`  ${e}`));
        }

        if (failedRequests.length > 0) {
          console.error('\n Network Failures:');
          failedRequests.forEach(r => console.error(`  ${r.method} ${r.url} -> ${r.status}${r.failure ? ` (${r.failure})` : ''}`));
        }
      });
    },
  });
}

export const monitoringTest = createMonitoringFixture();

export async function getPageWithMonitoring(page: Page): Promise<{ page: Page; monitoring: TestContext }> {
  const context: TestContext = {
    consoleMessages: [],
    networkRequests: [],
    jsErrors: [],
  };

  page.on('console', (msg) => {
    context.consoleMessages.push({
      type: msg.type() as ConsoleMessage['type'],
      text: msg.text(),
      location: msg.location()?.url || 'unknown',
    });
  });

  page.on('pageerror', (error) => {
    context.jsErrors.push(error.message);
  });

  page.on('response', (response) => {
    context.networkRequests.push({
      url: response.url(),
      method: response.request().method(),
      status: response.status(),
      failure: response.request().failure()?.errorText,
    });
  });

  return { page, monitoring: context };
}

export function assertNoErrors(monitoring: TestContext) {
  const isAuthMe401 = (r: NetworkRequest) =>
    r.status === 401 && /\/api\/(?:v1\/)?auth\/me(?:\?|$)/.test(r.url);
  const hasAuthMe401 = monitoring.networkRequests.some(isAuthMe401);
  const onlyAuthMe401s = hasAuthMe401 && monitoring.networkRequests.filter((r) => r.status === 401).every(isAuthMe401);

  const ignoredRequest = (r: NetworkRequest) => isAuthMe401(r);
  const ignoredConsole = (text: string) =>
    (text.includes('/api/auth/me') && text.includes('401')) ||
    (onlyAuthMe401s && text.includes('Failed to load resource') && text.includes('401 (Unauthorized)')) ||
    (text.includes('The final argument passed to') && text.includes('changed size between renders'));

  const errors = monitoring.consoleMessages.filter(
    (m) => m.type === 'error' && !ignoredConsole(m.text),
  );
  const jsErrors = monitoring.jsErrors;
  const failedRequests = monitoring.networkRequests.filter(
    (r) => (r.status >= 400 || r.failure) && !ignoredRequest(r),
  );

  const allErrors = [
    ...errors.map(e => `Console: ${e.text}`),
    ...jsErrors.map(e => `JS Error: ${e}`),
    ...failedRequests.map(r => `Network: ${r.method} ${r.url} -> ${r.status}`),
  ];

  if (allErrors.length > 0) {
    throw new Error(`Test produced errors:\n${allErrors.join('\n')}`);
  }
}
