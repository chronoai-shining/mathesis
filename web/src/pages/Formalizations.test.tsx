import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { it, expect, vi } from 'vitest';
import { fakeApi, renderWithApp } from '../test/render';
import { FormalizationsPage } from './Formalizations';
import type { FormalizationOpportunity } from '../api/types';

const opportunity: FormalizationOpportunity = {
  record: 'WP-2026-0001',
  claim: 'C1',
  title: 'Prime gaps',
  label: 'Theorem 1',
  statement: 'A checked mathematical statement',
  authors: [{ name: 'Research author' }],
  kind: 'paper',
  status: 'open',
  target_digest: 'exact-target',
  toolchain: 'pinned',
  verified_count: 0,
  contributor: null,
  attribution: null,
  receipt: null,
};
it('links to an attributed contribution and asks the API to filter the full directory', async () => {
  const list = vi.fn(async () => ({ items: [opportunity], next_before: null }));
  renderWithApp(<FormalizationsPage />, fakeApi({ formalizationOpportunities: list }));
  expect(await screen.findByRole('link', { name: 'Contribute a proof' })).toHaveAttribute(
    'href',
    '/submit?type=lean&record=WP-2026-0001&claim=C1',
  );
  expect(screen.getByText('Target ready')).toBeInTheDocument();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: 'Verified proofs' }));
  await user.selectOptions(screen.getByLabelText('Statement type'), 'paper');
  await user.type(screen.getByLabelText('Find a statement'), 'prime gaps');
  await user.click(screen.getByRole('button', { name: 'Search' }));
  await waitFor(() =>
    expect(list).toHaveBeenLastCalledWith(
      expect.objectContaining({ status: 'verified', kind: 'paper', q: 'prime gaps' }),
      expect.anything(),
    ),
  );
});
it('offers source intake when there are no confirmed targets yet', async () => {
  renderWithApp(<FormalizationsPage />, fakeApi());
  expect(await screen.findByText('The next proof can start with you.')).toBeInTheDocument();
  expect(screen.getByRole('link', { name: 'Submit existing Lean work →' })).toHaveAttribute(
    'href',
    '/submit?type=lean',
  );
});
