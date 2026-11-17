import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter, Routes, Route, useNavigate } from 'react-router-dom';
import { RouteErrorBoundaryWithLocation } from './route-error-boundary-with-location';

function Boom(): never {
  throw new Error('Boom');
}

function Safe() {
  return <div>Safe page</div>;
}

// Always-mounted navigation, like an app shell link: a user-facing click must
// clear the crashed boundary without any explicit reset call.
function NavButton() {
  const navigate = useNavigate();
  return (
    <button data-testid="go-safe" onClick={() => navigate('/safe')}>
      Go to safe page
    </button>
  );
}

function Harness() {
  return (
    <MemoryRouter initialEntries={['/crash']}>
      <NavButton />
      <RouteErrorBoundaryWithLocation>
        <Routes>
          <Route path="/crash" element={<Boom />} />
          <Route path="/safe" element={<Safe />} />
        </Routes>
      </RouteErrorBoundaryWithLocation>
    </MemoryRouter>
  );
}

describe('RouteErrorBoundaryWithLocation', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders the fallback on a route crash', () => {
    render(<Harness />);
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.queryByText('Safe page')).toBeNull();
  });

  it('resets the boundary on navigation so the new route renders without "Try Again"', () => {
    render(<Harness />);
    expect(screen.getByRole('alert')).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('go-safe'));

    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.getByText('Safe page')).toBeInTheDocument();
  });
});
