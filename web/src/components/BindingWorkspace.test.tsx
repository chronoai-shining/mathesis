import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { accepted, author, contributor } from '../test/fixtures';
import { fakeApi, renderWithApp } from '../test/render';
import type { BindingOption, PublicBindingBoard } from '../api/types';
import { BindingWorkspace } from './BindingWorkspace';
import { BindingPage } from '../pages/Bindings';
const option: BindingOption = {
  digest: 'exact-option',
  target: {
    subject: {
      submission: accepted.id,
      version: 1,
      claims_revision: 1,
      round: 1,
      claim: 'C1',
      source_digest: 'source',
      claim_digest: 'claim',
    },
    statement: 'For all n, the named bound holds.',
    sources: [
      {
        snapshot: 'snapshot',
        module_path: 'Fixture.lean',
        declaration_name_key: 'Existing.bound',
        statement_id: 'statement',
        type_digest: 'type',
      },
    ],
  },
};
const board: PublicBindingBoard = {
  id: 'board',
  target: option.target,
  context: 'Approved context',
  license: 'CC-BY-4.0',
  authorized_by: author.id,
  authorized_at: '2026-10-16T00:00:00Z',
  contributions: [],
  challenges: [],
  audits: [],
  credits: [],
};
it('requires separate unchecked consent and exact selected sources before sharing', async () => {
  const user = userEvent.setup();
  const publishBinding = vi.fn(async () => board);
  const api = fakeApi(
    { privateBindings: vi.fn(async () => ({ options: [option], boards: [] })), publishBinding },
    author,
  );
  renderWithApp(<BindingWorkspace submission={accepted} />, api);
  await user.click(screen.getByText('Public binding discussions'));
  await user.click(await screen.findByText('Share C1 for a binding discussion'));
  const publish = screen.getByRole('button', { name: 'Publish this summary' });
  expect(publish).toBeDisabled();
  await user.type(screen.getByLabelText('Public mathematical context'), 'Approved context');
  await user.click(screen.getByRole('checkbox', { name: /Existing.bound/ }));
  const consent = screen.getByRole('checkbox', { name: /I authorize public sharing/ });
  expect(consent).not.toBeChecked();
  await user.click(consent);
  await user.type(screen.getByLabelText('Public mathematical context'), ' revised');
  expect(consent).not.toBeChecked();
  await user.click(consent);
  await user.click(publish);
  await waitFor(() =>
    expect(publishBinding).toHaveBeenCalledWith(accepted.id, {
      option_digest: 'exact-option',
      sources: option.target.sources,
      context: 'Approved context revised',
      license: 'CC-BY-4.0',
      consent: 'binding-summary-2026-10-16',
    }),
  );
});
it('submits structured premises and a licensed argument without a client verdict', async () => {
  const user = userEvent.setup();
  const contributeBinding = vi.fn(async () => board);
  const api = fakeApi({ binding: vi.fn(async () => board), contributeBinding }, contributor);
  renderWithApp(<BindingPage id={accepted.id} boardKey="board" />, api);
  await screen.findByText('Contribute a joint binding');
  await user.click(screen.getByRole('checkbox', { name: 'Existing.bound' }));
  await user.type(screen.getByLabelText(/Parameter substitutions/), 'x => n + 1');
  await user.type(
    screen.getByLabelText('Joint mathematical argument'),
    'Apply the exact source with this parameter correspondence.',
  );
  await user.click(screen.getByRole('checkbox', { name: /I have the right/ }));
  await user.click(screen.getByRole('button', { name: 'Submit for independent inspection' }));
  await waitFor(() =>
    expect(contributeBinding).toHaveBeenCalledWith(
      accepted.id,
      'board',
      expect.objectContaining({
        binding: expect.objectContaining({
          premises: option.target.sources,
          parameters: [{ parameter: 'x', expression: 'n + 1' }],
          unresolved: [],
        }),
        consent: 'binding-contribution-2026-10-16',
        license: 'CC-BY-4.0',
      }),
    ),
  );
  expect(screen.getByText('No currently supported binding has earned credit.')).toBeInTheDocument();
});
it('retains an explicit contribution identity when challenging or appealing', async () => {
  const user = userEvent.setup();
  const contribution = {
    id: 'contribution-1',
    canonical: 'canonical',
    contributor: contributor.id,
    binding: {
      premises: option.target.sources,
      relation: 'implies' as const,
      parameters: [],
      hypotheses: [],
      argument: 'Proposed binding',
      unresolved: [],
      source_lineage: [],
    },
    license: 'CC-BY-4.0',
    consent: 'binding-contribution-2026-10-16',
    at: board.authorized_at,
  };
  const view = { ...board, contributions: [contribution] };
  const challengeBinding = vi.fn(async () => view);
  const api = fakeApi({ binding: vi.fn(async () => view), challengeBinding }, contributor);
  renderWithApp(<BindingPage id={accepted.id} boardKey="board" />, api);
  await user.click(await screen.findByText('Challenge this binding or appeal its inspection'));
  await user.type(
    screen.getByLabelText('Reproducible challenge or appeal evidence'),
    'The zero input violates the stated hypothesis.',
  );
  await user.click(screen.getAllByRole('checkbox', { name: /I have the right/ })[0]!);
  await user.click(screen.getByRole('button', { name: 'Request independent reinspection' }));
  await waitFor(() =>
    expect(challengeBinding).toHaveBeenCalledWith(accepted.id, 'board', {
      contribution: 'contribution-1',
      evidence: 'The zero input violates the stated hypothesis.',
      license: 'CC-BY-4.0',
      consent: 'binding-contribution-2026-10-16',
    }),
  );
});
