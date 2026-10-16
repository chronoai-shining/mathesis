import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { accepted, author, contributor } from '../test/fixtures';
import { fakeApi, renderWithApp } from '../test/render';
import type { FeedbackPreview, FeedbackView } from '../api/types';
import { FeedbackWorkspace } from './FeedbackWorkspace';
import { FeedbackPage } from '../pages/Feedback';
const preview: FeedbackPreview = {
  digest: 'sha256:exact',
  producer: 'mathesis:open-problem:fixture',
  source: {
    submission: accepted.id,
    record: 'WP-2026-0001',
    version: 1,
    claims_revision: 1,
    source_digest: 'source',
  },
  kind: { kind: 'open_problem', claim: 'C1' },
  required_signers: [author.id, contributor.id],
  license: 'Apache-2.0',
  payload: { evidence: 'Exact public statement', original_license: 'preserved' },
};
const view: FeedbackView = {
  proposal: {
    ...preview,
    id: preview.digest,
    payload: { id: 'blob', bytes: 100, sha256: 'exact' },
    prepared_by: author.id,
    prepared_at: '2026-10-16T00:00:00Z',
    preparation_consent: 'feedback-prepare-2026-10-16',
    events: [],
  },
  payload: preview.payload,
  state: 'awaiting_authorizations',
};
it('requires a reviewed preview and separate unchecked preparation consent', async () => {
  const user = userEvent.setup();
  const prepareFeedback = vi.fn(async () => view);
  const authorizeFeedback = vi.fn(async () => view);
  renderWithApp(
    <FeedbackWorkspace submission={accepted} />,
    fakeApi(
      { previewFeedback: vi.fn(async () => preview), prepareFeedback, authorizeFeedback },
      author,
    ),
  );
  await user.click(screen.getByText('Authorized knowledge proposals'));
  await user.click(screen.getByRole('button', { name: 'Preview exact evidence' }));
  const save = await screen.findByRole('button', { name: 'Save proposal for authorization' });
  expect(save).toBeDisabled();
  const checkbox = screen.getByRole('checkbox', { name: /I approve preparing/ });
  expect(checkbox).not.toBeChecked();
  await user.click(checkbox);
  await user.click(save);
  await waitFor(() =>
    expect(prepareFeedback).toHaveBeenCalledWith(accepted.id, {
      kind: preview.kind,
      preview_digest: preview.digest,
      preparation_consent: 'feedback-prepare-2026-10-16',
    }),
  );
  expect(authorizeFeedback).not.toHaveBeenCalled();
});
it('requires the contributor’s own digest-bound Apache grant before exporting', async () => {
  const user = userEvent.setup();
  const authorizeFeedback = vi.fn(async () => view);
  const exportFeedback = vi.fn();
  renderWithApp(
    <FeedbackPage id={accepted.id} proposalKey={view.proposal.id} />,
    fakeApi({ feedback: vi.fn(async () => view), authorizeFeedback, exportFeedback }, contributor),
  );
  const authorize = await screen.findByRole('button', { name: 'Authorize this exact package' });
  expect(authorize).toBeDisabled();
  expect(
    screen.queryByRole('button', { name: 'Download authorized proposal' }),
  ).not.toBeInTheDocument();
  await user.click(screen.getByRole('checkbox', { name: /I hold the applicable rights/ }));
  await user.click(authorize);
  await waitFor(() =>
    expect(authorizeFeedback).toHaveBeenCalledWith(accepted.id, view.proposal.id, {
      digest: preview.digest,
      license: 'Apache-2.0',
      consent: 'feedback-export-2026-10-16',
      rights_asserted: true,
    }),
  );
  expect(exportFeedback).not.toHaveBeenCalled();
});
it('closes stale payload access and presents merge, declaration and provenance facts separately', async () => {
  const observation = {
    request: 'request',
    pull_request: 9,
    upstream_repository: 'the-omega-institute/trureturing',
    upstream_base: 'dev',
    reference_state: 'merged',
    merge_commit: 'merge',
    merged_at: '2026-10-16T00:00:00Z',
    consumed_source_commit: 'source',
    consumed_source_tree: 'tree',
    snapshot: 'snapshot',
    index_revision: 'index',
    merge_in_consumed_source: false,
    declarations: [
      {
        identity: {
          snapshot: 'snapshot',
          module_path: 'Fixture.lean',
          declaration_name_key: 'Fixture.theorem',
          statement_id: 'statement',
          type_digest: 'type',
        },
        present: true,
        source_sha256: 'source',
        freeze_status: 'proven_not_frozen',
        frozen_node_id: null,
        package_marker_present: false,
      },
    ],
    evidence: { id: 'evidence', bytes: 4, sha256: 'hash' },
    observed_at: '2026-10-16T00:00:00Z',
    limits: ['Fixture only; no mathematical correspondence established.'],
  };
  const stale: FeedbackView = {
    ...view,
    payload: null,
    state: 'source_or_evidence_changed',
    proposal: { ...view.proposal, events: [{ event: 'observed', observation }] },
  };
  renderWithApp(
    <FeedbackPage id={accepted.id} proposalKey={view.proposal.id} />,
    fakeApi({ feedback: vi.fn(async () => stale) }, author),
  );
  await screen.findByText('PR #9: merged');
  expect(screen.getByText('Included in consumed source: Not established')).toBeInTheDocument();
  expect(
    screen.getByText(
      /Fixture.theorem: Exact declaration found; module proposal provenance: Not established; original freeze status: proven_not_frozen/,
    ),
  ).toBeInTheDocument();
  expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
  expect(
    screen.queryByRole('button', { name: 'Download authorized proposal' }),
  ).not.toBeInTheDocument();
  expect(screen.queryByText(/Exact public statement/)).not.toBeInTheDocument();
});
