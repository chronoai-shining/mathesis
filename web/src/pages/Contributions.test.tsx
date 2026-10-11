import { readFileSync } from 'node:fs';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { Entrant, ErrorReport, StatementDetail, VerifiedAttempt } from '../api/types';
import { ReportError } from '../components/ErrorReports';
import { author, paperSummary } from '../test/fixtures';
import { fakeApi, renderWithApp } from '../test/render';
import { StatementPage, SupportersPage } from './Contributions';

const agent: Entrant = {
  id: 'agent:one',
  name: 'Proof agent',
  kind: 'agent',
  owner: { id: author.id, name: author.display_name },
  retired: false,
  revision: 0,
};
const report: ErrorReport = {
  id: 'report-one',
  record: paperSummary.record,
  reporter: agent,
  subject: { kind: 'statement', claim: 'C1' },
  text: '',
  resolution: null,
  created_at: '2026-10-11T00:00:00Z',
};

describe('phase D contributions', () => {
  it('offers reports only when signed in', () => {
    renderWithApp(<ReportError record={report.record} subject={report.subject} />, fakeApi());
    expect(screen.queryByRole('button', { name: 'Report an error' })).not.toBeInTheDocument();
  });
  it('accepts optional Lean-only evidence as an owned active agent and explains privacy', async () => {
    const submit = vi.fn(async () => report);
    const user = userEvent.setup();
    renderWithApp(
      <ReportError record={report.record} subject={report.subject} />,
      fakeApi(
        {
          reportError: submit,
          agents: vi.fn(async () => [
            agent,
            { ...agent, id: 'retired', name: 'Retired agent', retired: true },
          ]),
        },
        author,
      ),
    );
    await user.click(await screen.findByRole('button', { name: 'Report an error' }));
    expect(screen.getByText(/Confirmed findings become public/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Submit report' }));
    expect(submit).not.toHaveBeenCalled();
    expect(await screen.findByRole('alert')).toHaveTextContent('Add evidence');
    await screen.findByRole('option', { name: agent.name });
    expect(screen.queryByRole('option', { name: 'Retired agent' })).not.toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText('Report as'), agent.id);
    const lean = 'import Target\ntheorem wishpool_disproof : ¬ wishpool_target_prop := by simp';
    const file = new File([lean], 'Disproof.lean', { type: 'text/plain' });
    Object.defineProperty(file, 'text', { value: async () => lean });
    await user.upload(screen.getByLabelText('Lean disproof (optional)'), file);
    await user.click(screen.getByRole('button', { name: 'Submit report' }));
    expect(submit).toHaveBeenCalledWith(report.record, {
      subject: report.subject,
      text: '',
      solution: lean,
      as_agent: agent.id,
    });
    expect(await screen.findByRole('status')).toHaveTextContent(
      'It stays private while we check it.',
    );
  });
  it('requires text for a target report and does not offer a Lean attachment', async () => {
    const submit = vi.fn(async () => report);
    const user = userEvent.setup();
    const subject = { kind: 'target' as const, claim: 'C1', digest: 'a'.repeat(64) };
    renderWithApp(
      <ReportError record={report.record} subject={subject} />,
      fakeApi({ reportError: submit }, author),
    );
    await user.click(await screen.findByRole('button', { name: 'Report an error' }));
    expect(screen.queryByLabelText('Lean disproof (optional)')).not.toBeInTheDocument();
    expect(screen.getByLabelText('Evidence')).toHaveAttribute('maxlength', '20000');
    await user.type(screen.getByLabelText('Evidence'), 'The target omits the hypothesis n > 0.');
    await user.click(screen.getByRole('button', { name: 'Submit report' }));
    expect(submit).toHaveBeenCalledWith(report.record, {
      subject,
      text: 'The target omits the hypothesis n > 0.',
      solution: undefined,
      as_agent: undefined,
    });
  });
  it('shows unranked receipt-metered supporters at 390px with wrapping controls', async () => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 390 });
    renderWithApp(
      <SupportersPage />,
      fakeApi({
        supporters: vi.fn(async () => [
          { name: 'A'.repeat(180), tokens: 0 },
          { name: 'Zoe', tokens: 125 },
        ]),
      }),
    );
    expect(await screen.findByText('0 metered tokens')).toBeInTheDocument();
    expect(screen.getByText('125 metered tokens')).toBeInTheDocument();
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
    expect(document.querySelector('.supporter-list')?.tagName).toBe('UL');
    const css = readFileSync('src/styles/app.css', 'utf8');
    expect(css).toMatch(
      /\.supporter-list,\s*\.report-text\s*\{[^}]*min-width:\s*0;[^}]*overflow-wrap:\s*anywhere/s,
    );
    expect(css).toMatch(/\.supporter-list > li\s*\{[^}]*flex-wrap:\s*wrap/s);
    expect(css).toMatch(
      /@media \(max-width: 480px\)[\s\S]*\.error-report textarea\s*\{[^}]*max-width:\s*100%;[^}]*min-width:\s*0/s,
    );
  });
  it('credits a verified paper statement and describes the exact confirmed target', async () => {
    const proof: VerifiedAttempt = {
      id: 'proof',
      entrant: agent,
      solution: 'checked source',
      also_verified: false,
      receipt: {
        verdict: 'proved',
        reason: 'checked',
        target_digest: 'a'.repeat(64),
        solution_digest: 'b'.repeat(64),
        toolchain: 'Lean test',
        axioms: [],
        checked_at: report.created_at,
        duration: 1,
      },
    };
    const statement: StatementDetail = {
      record: report.record,
      title: 'Paper',
      macros: {},
      claim: {
        id: 'C1',
        kind: 'theorem',
        role: 'main',
        label: 'Theorem 1',
        statement: 'True',
        section: undefined,
        depends_on: [],
        lean: null,
        verified: proof,
      },
      target: {
        claim: 'C1',
        digest: proof.receipt.target_digest,
        lean: 'import Mathlib\ndef wishpool_target_prop : Prop := True',
        toolchain: 'Lean test',
      },
      verified_attempts: [proof],
    };
    renderWithApp(
      <StatementPage record={report.record} claim="C1" />,
      fakeApi({ getStatement: vi.fn(async () => statement) }),
    );
    expect(await screen.findByText('Lean ✓')).toBeInTheDocument();
    expect(screen.getByText(/proved the author-confirmed Lean target\./)).toBeInTheDocument();
    expect(screen.getByText('Reviewed by GPT Pro and audited by Codex')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Download Target.lean' })).toHaveAttribute(
      'href',
      `/api/v1/papers/${report.record}/statements/C1/target`,
    );
    expect(screen.getAllByRole('link', { name: agent.name })).toHaveLength(2);
  });
});
