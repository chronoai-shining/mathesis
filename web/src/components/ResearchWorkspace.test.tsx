import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { accepted, author, contributor } from '../test/fixtures';
import { fakeApi, renderWithApp } from '../test/render';
import type { ResearchCandidate, ResearchView } from '../api/types';
import { ResearchWorkspace } from './ResearchWorkspace';

const candidate: ResearchCandidate = {
  digest: 'sha256:' + 'a'.repeat(64),
  subject: {
    submission: accepted.id,
    version: 1,
    claims_revision: 1,
    round: 1,
    claim: 'C1',
    source_digest: 'source',
    claim_digest: 'claim',
  },
  gap: { kind: 'hypothesis', index: 0 },
  unresolved_text: 'Prove positivity',
  original_statement: 'The original claim',
  receipt_ids: ['query-1'],
  binding: {
    premises: [
      {
        snapshot: 'pin',
        module_path: 'Fixture.lean',
        declaration_name_key: 'lemma',
        statement_id: 'statement',
        type_digest: 'type',
      },
    ],
    relation: 'implies',
    parameters: [],
    hypotheses: [{ hypothesis: 'x > 0', argument: '' }],
    argument: 'The lemma requires positivity.',
    unresolved: [],
    source_lineage: [],
  },
};
const view: ResearchView = {
  candidates: [],
  obligations: [
    {
      state: 'awaiting_confirmation',
      attempts: [],
      obligation: {
        id: 'research-1',
        candidate,
        goal: {
          definitions: [],
          hypotheses: [],
          quantifiers: 'For every x',
          conclusion: 'x > 0',
          correspondence: 'Required premise',
        },
        budget: { max_attempts: 2, expires_at: '2026-10-20T00:00:00Z' },
        authorized_at: '2026-10-16T00:00:00Z',
        authorized_by: author.id,
        consent: 'research-obligation-2026-10-16',
      },
      target: {
        claim: 'C1',
        version: 1,
        claims_revision: 1,
        lean: 'import Mathlib\ndef wishpool_target_prop : Prop := True',
        digest: 'exact-digest',
        toolchain: 'test-toolchain',
        reading: 'The exact goal',
        created_at: '2026-10-16T00:00:00Z',
        response: { state: 'awaiting_author' },
        faithfulness: {
          verdict: 'faithful',
          reason: 'Independent fixture inspection',
          mismatch: '',
          regenerated: false,
        },
      },
    },
  ],
};
async function open() {
  await userEvent.click(screen.getByText('Private research goals'));
}
it('requires explicit unchecked consent and sends the exact candidate pin and full goal', async () => {
  const adoptResearch = vi.fn(async () => view);
  renderWithApp(
    <ResearchWorkspace submission={accepted} />,
    fakeApi(
      {
        research: vi.fn(async () => ({ candidates: [candidate], obligations: [] })),
        adoptResearch,
      },
      author,
    ),
  );
  await open();
  await userEvent.click(await screen.findByText('C1 · Prove positivity'));
  const consent = screen.getByRole('checkbox');
  expect(consent).not.toBeChecked();
  await userEvent.type(
    screen.getByLabelText('Variables and quantifiers'),
    'For every x in the domain',
  );
  await userEvent.type(screen.getByLabelText('Exact conclusion to prove or refute'), 'x > 0');
  await userEvent.type(
    screen.getByLabelText('How this goal addresses the audited gap'),
    'Discharge the positivity premise',
  );
  await userEvent.click(screen.getByRole('button', { name: 'Authorize private research' }));
  expect(adoptResearch).not.toHaveBeenCalled();
  await userEvent.click(consent);
  await userEvent.click(screen.getByRole('button', { name: 'Authorize private research' }));
  await waitFor(() =>
    expect(adoptResearch).toHaveBeenCalledWith(
      accepted.id,
      expect.objectContaining({
        candidate_digest: candidate.digest,
        gap: candidate.gap,
        consent: 'research-obligation-2026-10-16',
        goal: expect.objectContaining({ conclusion: 'x > 0' }),
      }),
    ),
  );
});
it('confirms the displayed digest only after a faithful inspection', async () => {
  const respondResearchTarget = vi.fn(async () => view);
  renderWithApp(
    <ResearchWorkspace submission={accepted} />,
    fakeApi({ research: vi.fn(async () => view), respondResearchTarget }, author),
  );
  await open();
  await userEvent.click(await screen.findByRole('button', { name: 'Confirm this exact target' }));
  await waitFor(() =>
    expect(respondResearchTarget).toHaveBeenCalledWith(accepted.id, 'research-1', {
      digest: 'exact-digest',
      confirm: true,
    }),
  );
});
it('keeps nonfaithful targets unconfirmable and hides mutations from other readers', async () => {
  const mismatch: ResearchView = structuredClone(view);
  mismatch.obligations[0]!.target!.faithfulness!.verdict = 'weaker';
  const { unmount } = renderWithApp(
    <ResearchWorkspace submission={accepted} />,
    fakeApi({ research: vi.fn(async () => mismatch) }, author),
  );
  await open();
  expect(await screen.findByRole('button', { name: 'Confirm this exact target' })).toBeDisabled();
  unmount();
  renderWithApp(
    <ResearchWorkspace submission={accepted} />,
    fakeApi({ research: vi.fn(async () => view) }, contributor),
  );
  await open();
  await screen.findByText('The exact goal');
  expect(screen.queryByRole('button', { name: 'Confirm this exact target' })).toBeNull();
  expect(screen.queryByRole('button', { name: 'Stop research' })).toBeNull();
});
