import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TopNav } from './top-nav';

// Mock dependencies
vi.mock('@/stores/useAppStore', () => ({
  useAppStore: vi.fn(() => ({
    user: { id: '1', email: 'test@example.com', full_name: 'Test User' },
    currentWorkspace: { id: 'kb-1', title: 'Test Workspace', user_id: '1', created_at: '2024-01-01' },
    searchDialogOpen: false,
    toggleSearchDialog: vi.fn(),
  })),
}));

vi.mock('@/components/ui/button', () => ({
  Button: ({ children, ...props }: any) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock('@/components/ui/dropdown-menu', () => ({
  DropdownMenu: ({ children }: any) => <div>{children}</div>,
  DropdownMenuTrigger: ({ children, asChild }: any) => <div>{asChild ? children : <button>Trigger</button>}</div>,
  DropdownMenuContent: ({ children }: any) => <div>{children}</div>,
  DropdownMenuItem: ({ children }: any) => <div>{children}</div>,
  DropdownMenuLabel: ({ children }: any) => <div>{children}</div>,
  DropdownMenuSeparator: () => <div>separator</div>,
}));

describe('TopNav', () => {
  it('should render logo', () => {
    render(<TopNav />);
    expect(screen.getByText('Context OS')).toBeInTheDocument();
  });

  it('should render search button', () => {
    render(<TopNav />);
    expect(screen.getByText('搜索')).toBeInTheDocument();
    expect(screen.getByText('⌘K')).toBeInTheDocument();
  });

  it('should render workspace name', () => {
    render(<TopNav />);
    expect(screen.getByText('Test Workspace')).toBeInTheDocument();
  });

  it('should render user initial', () => {
    render(<TopNav />);
    // User initial T from Test User
    expect(screen.getByText('T')).toBeInTheDocument();
  });
});
