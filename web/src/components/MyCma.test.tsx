import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { MyCmaStatus } from '../api/types';
import { donor } from '../test/fixtures';
import { fakeApi, renderWithApp } from '../test/render';
import { CmaHistory, MyCma } from './MyCma';

const binding: MyCmaStatus = {
  enabled: true,
  setup_url: 'https://cma.example/auth/login?return_to=/',
  binding: {
    person: donor.id,
    subject: donor.id,
    revision: 2,
    generation: 1,
    status: 'setup_needed',
    reason: 'workspace_needed',
    workspace_id: null,
    sandbox_id: null,
    sandbox_profile: null,
    checked_at: null,
    created_at: '2026-10-11T00:00:00Z',
    updated_at: '2026-10-11T00:00:00Z',
  },
};
describe('My CMA account', () => {
  it('encourages author funding and preserves the operator default without sign-in', async () => {
    const api = fakeApi();
    renderWithApp(<MyCma />, api);
    expect(await screen.findByText('Sign in to connect your CMA account.')).toBeInTheDocument();
    expect(screen.getByText(/The GPT Pro referee is paid by Wishpool/)).toBeInTheDocument();
    expect(screen.getByText(/each step shows who paid/)).toBeInTheDocument();
    expect(api.getCma).not.toHaveBeenCalled();
  });
  it('validates a workspace with the binding revision, rechecks and disconnects', async () => {
    const ready: MyCmaStatus = {
      ...binding,
      binding: {
        ...binding.binding,
        status: 'ready',
        revision: 3,
        reason: 'author_connected',
        workspace_id: 'wks_mine',
      },
    };
    const setCmaWorkspace = vi.fn(async () => ready);
    const disconnectCma = vi.fn(async () => ({
      ...binding,
      binding: {
        ...binding.binding,
        status: 'disconnected' as const,
        reason: 'not_connected',
        generation: 2,
        revision: 4,
      },
    }));
    renderWithApp(
      <MyCma />,
      fakeApi({ getCma: vi.fn(async () => binding), setCmaWorkspace, disconnectCma }, donor),
    );
    expect(await screen.findByText('Setup needed')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open CMA setup' })).toHaveAttribute(
      'href',
      binding.setup_url,
    );
    expect(screen.getByRole('link', { name: 'Connect CMA' })).toHaveAttribute(
      'href',
      expect.stringContaining('/auth/cma/connect?return_to='),
    );
    await userEvent.type(screen.getByLabelText('CMA workspace ID'), 'wks_mine');
    await userEvent.click(screen.getByRole('button', { name: 'Save / recheck workspace' }));
    expect(setCmaWorkspace).toHaveBeenLastCalledWith({
      workspace_id: 'wks_mine',
      expected_revision: 2,
    });
    expect(await screen.findByText('Connected')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Save / recheck workspace' }));
    expect(setCmaWorkspace).toHaveBeenLastCalledWith({
      workspace_id: 'wks_mine',
      expected_revision: 3,
    });
    await userEvent.click(screen.getByRole('button', { name: 'Disconnect CMA' }));
    expect(await screen.findByText('Not connected')).toBeInTheDocument();
    expect(screen.queryByLabelText('CMA workspace ID')).toBeNull();
  });
  it('shows explicit payer successors before any letter', async () => {
    renderWithApp(
      <CmaHistory
        attempts={[
          {
            step: 'r1:audit',
            attempt: 0,
            payer: 'author',
            reason: 'author_connected',
            status: 'failed',
            admitted_at: '2026-10-11T00:00:00Z',
            finished_at: '2026-10-11T00:01:00Z',
          },
          {
            step: 'r1:audit',
            attempt: 1,
            payer: 'operator',
            reason: 'cause_unknown',
            status: 'running',
            admitted_at: '2026-10-11T00:01:00Z',
            finished_at: null,
          },
        ]}
      />,
      fakeApi(),
    );
    await screen.findByText(
      'The GPT Pro referee is paid by Wishpool. Each CMA attempt below keeps its recorded payer.',
    );
    const list = within(screen.getByRole('region', { name: 'CMA review funding' })).getAllByRole(
      'listitem',
    );
    expect(list[0]).toHaveTextContent('Author account');
    expect(list[1]).toHaveTextContent('Wishpool');
    expect(list[1]).toHaveTextContent('unknown cause; Wishpool fallback');
    expect(document.body).not.toHaveTextContent('workspace');
  });
});
