import { cleanup, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { ApiError } from '../api/client';
import type { Person } from '../api/types';
import { CMA_COPY } from '../components/MyCma';
import { author, draft, editor, submission } from '../test/fixtures';
import { fakeApi, renderWithApp } from '../test/render';
import { ContributePage } from './Contribute';
import { SubmissionDetailPage } from './SubmissionDetail';
import { SubmitPage } from './Submit';

const privateFields = {
  workspace: 'PRIVATE_WORKSPACE_SENTINEL',
  owner: 'PRIVATE_OWNER_SENTINEL',
  binding_generation: 'PRIVATE_GENERATION_SENTINEL',
  progress: 'PRIVATE_PROGRESS_SENTINEL',
  access_token: 'PRIVATE_ACCESS_SENTINEL',
  refresh_token: 'PRIVATE_REFRESH_SENTINEL',
  sealed: 'PRIVATE_SEALED_SENTINEL',
};
const attempts = [
  {
    ...privateFields,
    kind: 'audit' as const,
    round: 1,
    step: 'r1:audit',
    attempt: 0,
    payer: 'author' as const,
    reason: 'author_connected',
    status: 'running' as const,
    admitted_at: '2026-10-11T00:00:00Z',
    finished_at: null,
  },
];

describe('CMA funding in pages', () => {
  it.each(['submitter', 'coauthor', 'staff', 'public'] as const)(
    'shows pre-letter history to %s with submitter-only connection controls and safe fields',
    async (role) => {
      const coauthor = { ...author, id: 'linked-coauthor', display_name: 'Linked author' };
      const readers: Record<string, Person | undefined> = {
        submitter: author,
        coauthor,
        staff: editor,
        public: undefined,
      };
      const paper = {
        ...submission,
        authors: [...submission.authors, { name: coauthor.display_name, person: coauthor.id }],
      };
      const getCma = vi.fn(async () => ({
        enabled: true,
        setup_url: 'https://cma.example/auth/login?return_to=/',
        binding: {
          person: author.id,
          subject: author.id,
          revision: 0,
          generation: 0,
          status: 'disconnected' as const,
          reason: 'not_connected',
          workspace_id: null,
          sandbox_id: null,
          sandbox_profile: null,
          checked_at: null,
          created_at: '2026-10-11T00:00:00Z',
          updated_at: '2026-10-11T00:00:00Z',
          ...privateFields,
        },
      }));
      const referee = vi.fn(async () => ({
        id: paper.id,
        revision: 1,
        rounds: [],
        letters: [],
        cma_attempts: attempts,
        ...privateFields,
      }));
      renderWithApp(
        <SubmissionDetailPage id={paper.id} />,
        fakeApi({ getSubmission: vi.fn(async () => paper), referee, getCma }, readers[role]),
      );
      await screen.findByRole('heading', { name: paper.title });
      if (role === 'public') {
        expect(referee).not.toHaveBeenCalled();
        expect(screen.queryByRole('region', { name: 'CMA review funding' })).toBeNull();
      } else {
        expect(await screen.findByRole('region', { name: 'CMA review funding' })).toHaveTextContent(
          'Author account',
        );
        expect(referee).toHaveBeenCalled();
      }
      if (role === 'submitter') {
        expect(await screen.findByRole('link', { name: 'Connect CMA' })).toBeInTheDocument();
        expect(getCma).toHaveBeenCalled();
      } else {
        expect(screen.queryByRole('link', { name: 'Connect CMA' })).toBeNull();
        expect(getCma).not.toHaveBeenCalled();
      }
      for (const sentinel of Object.values(privateFields)) {
        expect(document.body.innerHTML).not.toContain(sentinel);
      }
    },
  );

  it('encourages BYOK on Submit and Contribute', async () => {
    for (const page of [<SubmitPage key="submit" />, <ContributePage key="contribute" />]) {
      renderWithApp(page, fakeApi({}, author));
      expect(await screen.findByText(CMA_COPY)).toBeInTheDocument();
      cleanup();
    }
  });

  it('uploads while the CMA status request fails', async () => {
    const createSubmission = vi.fn(async () => draft);
    const getCma = vi.fn(async () => {
      throw new ApiError({ status: 503, code: 'unavailable', detail: 'CMA status unavailable' });
    });
    renderWithApp(<SubmitPage />, fakeApi({ getCma, createSubmission }, author));
    const user = userEvent.setup();
    await screen.findByText('CMA status unavailable');
    const file = new File(['source'], 'paper.tex', { type: 'application/x-tex' });
    await user.upload(await screen.findByLabelText('LaTeX source'), file);
    await user.type(screen.getByLabelText('AI disclosure statement'), 'No AI used.');
    await user.click(screen.getByRole('button', { name: 'Upload and read statements' }));
    await waitFor(() => expect(createSubmission).toHaveBeenCalledWith(expect.any(Object), file));
  });
});
