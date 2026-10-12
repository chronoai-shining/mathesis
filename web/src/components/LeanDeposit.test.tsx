import { screen, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { it, expect, vi } from 'vitest';
import { fakeApi, renderWithApp } from '../test/render';
import { author } from '../test/fixtures';
import { SubmitPage } from '../pages/Submit';
import type { NewLeanDeposit } from '../api/types';

it('deposits source and original attribution without presenting it as verified', async () => {
  const submitFormalization = vi.fn(async (body: NewLeanDeposit) => ({
    ...body,
    id: 'lean-test',
    owner: author.id,
    source: { id: 'blob', bytes: 12, sha256: 'digest' },
    submitted_at: '2026-10-13T00:00:00Z',
    attempts: [],
    revision: 0,
  }));
  const api = fakeApi({ submitFormalization }, author);
  renderWithApp(<SubmitPage />, api);
  const user = userEvent.setup();
  await screen.findByLabelText('LaTeX source');
  await user.click(screen.getByRole('button', { name: 'Lean formalization' }));
  await user.type(screen.getByLabelText('Title', { exact: true }), 'A formal proof');
  await user.clear(screen.getByLabelText('Original authors (one per line)'));
  await user.type(screen.getByLabelText('Original authors (one per line)'), 'Original author');
  await user.type(screen.getByLabelText('Mathematical statement'), 'True');
  await user.type(
    screen.getByLabelText('Original research or source URL'),
    'https://example.org/proof.lean',
  );
  await user.type(screen.getByLabelText('Lean / Mathlib version pin'), 'test-toolchain');
  await user.type(screen.getByLabelText('Source license'), 'Apache-2.0');
  const file = new File(['import Target'], 'Solution.lean', { type: 'text/plain' });
  Object.defineProperty(file, 'text', { value: async () => 'import Target' });
  await user.upload(screen.getByLabelText('Lean source (up to 1 MB)'), file);
  // jsdom does not validate userEvent's synthetic FileList correctly; native submission is covered by browser QA.
  fireEvent.submit(screen.getByRole('form', { name: 'Submit Lean formalization' }));
  await waitFor(() =>
    expect(submitFormalization).toHaveBeenCalledWith(
      expect.objectContaining({
        authors: ['Original author'],
        solution: 'import Target',
        toolchain: 'test-toolchain',
      }),
    ),
  );
  expect(await screen.findByRole('status')).toHaveTextContent('Verification has not run yet');
  expect(api.verifyFormalization).not.toHaveBeenCalled();
});
