import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { author, submission } from '../test/fixtures';
import { refereeAudit, refereeFile, refereeRound } from '../test/referee';
import { fakeApi, renderWithApp } from '../test/render';
import { ReviewFindings } from './ReviewFindings';

it('retains local evidence and binds an author response to the confirmed current source', async () => {
  const file = refereeFile({
    rounds: [
      {
        ...refereeRound,
        audit: {
          attempts: 1,
          state: {
            state: 'done',
            at: refereeRound.started_at,
            result: {
              ...refereeAudit,
              concerns: [
                {
                  concern: 'Boundary case',
                  status: 'confirmed',
                  note: 'The inequality fails.',
                  basis: 'offline',
                  evidence: 'At n = 0 the sides are 1 and 0.',
                  location: 'Lemma 2',
                },
              ],
            },
          },
        },
      },
    ],
  });
  const respondToFinding = vi.fn(async () => file);
  const onChange = vi.fn();
  renderWithApp(
    <ReviewFindings submission={submission} file={file} onChange={onChange} />,
    fakeApi({ respondToFinding }, author),
  );
  const user = userEvent.setup();
  await user.click(await screen.findByText('R1-A1'));
  expect(screen.getByText('At n = 0 the sides are 1 and 0.')).toBeInTheDocument();
  expect(screen.getByText('Reported location: Lemma 2')).toBeInTheDocument();
  expect(screen.getByText(/A response does not mark a finding resolved/)).toBeInTheDocument();
  await user.type(
    await screen.findByLabelText('Your response or correction'),
    'The domain is n > 0.',
  );
  await user.click(screen.getByRole('button', { name: 'Send response & request re-review' }));
  await waitFor(() => expect(onChange).toHaveBeenCalledWith(file));
  expect(respondToFinding).toHaveBeenCalledWith(submission.id, {
    round: 1,
    concern: 0,
    version: submission.versions.at(-1)!.number,
    claims_revision: submission.claims_revision,
    body: 'The domain is n > 0.',
  });
});

it('distinguishes re-reviewed responses from resolved findings', async () => {
  const response = {
    id: 'response1',
    round: 1,
    concern: 0,
    version: refereeRound.version,
    claims_revision: refereeRound.claims_revision,
    body: 'Here is the source.',
    author: author.id,
    at: refereeRound.started_at,
  };
  const file = refereeFile({
    finding_responses: [response],
    rounds: [refereeRound, { ...refereeRound, number: 2, response_ids: ['response1'] }],
  });
  renderWithApp(
    <ReviewFindings submission={submission} file={file} onChange={vi.fn()} />,
    fakeApi({}, author),
  );
  expect(
    await screen.findByText('Re-reviewed in round 2. Read that audit for the conclusion.'),
  ).toBeInTheDocument();
  expect(screen.queryByText('Resolved')).toBeNull();
});
