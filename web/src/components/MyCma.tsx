import { useCallback, useState, type FormEvent } from 'react';
import { useApi } from '../api/context';
import { useAsync } from '../api/useAsync';
import type { CmaAttemptView } from '../api/types';
import { useSession } from '../auth/session';
import { Async, DateText, Field, InlineError, SignInPrompt } from './ui';

export const CMA_COPY =
  'Bring your own CMA account to run the Codex review of your submissions on your own model subscription. This helps cover review costs. While it is not connected or cannot run a step, Wishpool covers that step during the early phase; each step shows who paid. The GPT Pro referee is paid by Wishpool.';

const STATUS_LABELS = {
  disconnected: 'Not connected',
  setup_needed: 'Setup needed',
  ready: 'Connected',
  unusable: 'Connection unavailable',
};
const REASONS: Record<string, string> = {
  not_connected: 'Connect your NyxID account to begin.',
  consent_pending: 'Finish consent with NyxID, then choose your CMA workspace.',
  workspace_needed:
    'Create a ready workspace in your own CMA Sandbox with model access set to own credentials, then paste its workspace ID below.',
  workspace_not_owned: 'Choose a workspace owned by your signed-in NyxID account.',
  workspace_not_ready: 'Wait for your CMA Sandbox and workspace to be ready, then recheck.',
  own_credentials_required: 'Use a CMA Sandbox profile with model access set to own credentials.',
  workspace_setup_needed: 'Open CMA and finish your workspace setup, then recheck.',
  authorization_refused: 'Reconnect to renew your CMA authorization.',
  model_credentials_refused: 'Open CMA and renew your model credentials, then recheck.',
  author_quota_refused: 'Your model quota was refused. Restore quota in CMA, then recheck.',
};

export function MyCma() {
  const api = useApi();
  const { person } = useSession();
  const load = useCallback((signal: AbortSignal) => api.getCma({ signal }), [api]);
  const resource = useAsync(person ? load : null);
  const [workspace, setWorkspace] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const mutate = async (operation: () => ReturnType<typeof api.getCma>) => {
    setBusy(true);
    setError(null);
    try {
      resource.replace(await operation());
    } catch (e) {
      setError(e);
    } finally {
      setBusy(false);
    }
  };
  const returnTo = window.location.pathname + window.location.search;
  return (
    <section aria-label="My CMA account">
      <h2>My CMA account</h2>
      <p>{CMA_COPY}</p>
      {!person ? (
        <SignInPrompt what="connect your CMA account" />
      ) : (
        <Async state={resource.state} onRetry={resource.reload}>
          {(status) => (
            <>
              <p role="status">{STATUS_LABELS[status.binding.status]}</p>
              {!status.enabled ? (
                <p>
                  Author CMA connections are not enabled on this deployment. Wishpool covers your
                  CMA review steps.
                </p>
              ) : (
                <>
                  <p>{REASONS[status.binding.reason]}</p>
                  <div className="button-row">
                    <a
                      className="button button-quiet"
                      href={`/auth/cma/connect?return_to=${encodeURIComponent(returnTo)}`}
                    >
                      Connect CMA
                    </a>
                    <a
                      className="button button-quiet"
                      href={status.setup_url}
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      Open CMA setup
                    </a>
                    {status.binding.status !== 'disconnected' ? (
                      <button
                        className="button button-quiet"
                        disabled={busy}
                        onClick={() => {
                          void mutate(() => api.disconnectCma());
                        }}
                      >
                        Disconnect CMA
                      </button>
                    ) : null}
                  </div>
                  {status.binding.status !== 'disconnected' ? (
                    <form
                      className="inline-form"
                      aria-label="CMA workspace"
                      onSubmit={(event: FormEvent) => {
                        event.preventDefault();
                        const id = workspace.trim() || status.binding.workspace_id || '';
                        void mutate(() =>
                          api.setCmaWorkspace({
                            workspace_id: id,
                            expected_revision: status.binding.revision,
                          }),
                        );
                      }}
                    >
                      <Field label="CMA workspace ID" htmlFor="cma-workspace">
                        <input
                          id="cma-workspace"
                          value={workspace}
                          placeholder={status.binding.workspace_id ?? 'wks_…'}
                          maxLength={200}
                          onChange={(event) => setWorkspace(event.target.value)}
                        />
                      </Field>
                      <button
                        className="button button-quiet"
                        type="submit"
                        disabled={busy || !(workspace.trim() || status.binding.workspace_id)}
                      >
                        Save / recheck workspace
                      </button>
                    </form>
                  ) : null}
                </>
              )}
            </>
          )}
        </Async>
      )}
      <InlineError error={error} />
    </section>
  );
}

const ATTEMPT_REASONS: Record<string, string> = {
  author_connected: 'Your connected account',
  not_connected: 'Author account not connected',
  byok_disabled: 'Author connections not enabled',
  legacy_unknown: 'Legacy step; funding details unavailable',
  account: 'Author account refused; Wishpool fallback',
  auth: 'Author authorization refused; Wishpool fallback',
  setup: 'Author setup unavailable; Wishpool fallback',
  credential: 'Author model credentials refused; Wishpool fallback',
  quota: 'Author quota refused; Wishpool fallback',
  cause_unknown: 'Author agent failed for an unknown cause; Wishpool fallback',
};

const STEP_LABELS = {
  audit: 'Codex audit',
  advice: 'Advice',
  letter: 'Letter',
  lean_probe: 'Lean probe',
  conjecture_target: 'Conjecture target',
  problem_audit: 'Paper problem audit',
  legacy_unknown: 'Earlier CMA step',
};

export function CmaHistory({ attempts }: { attempts: CmaAttemptView[] }) {
  if (attempts.length === 0) return null;
  return (
    <section aria-label="CMA review funding">
      <h2>CMA review funding</h2>
      <p>
        The GPT Pro referee is paid by Wishpool. Each CMA attempt below keeps its recorded payer.
      </p>
      <ul className="entry-list">
        {attempts.map((attempt, index) => (
          <li key={`${attempt.step}-${attempt.attempt}-${index}`}>
            <strong>{attempt.kind ? STEP_LABELS[attempt.kind] : attempt.step}</strong>
            {attempt.round ? ` · Round ${attempt.round}` : null}
            {attempt.claim ? ` · Statement ${attempt.claim}` : null} · Attempt {attempt.attempt + 1}{' '}
            · {attempt.payer === 'author' ? 'Author account' : 'Wishpool'} · {attempt.status}
            <p>
              {ATTEMPT_REASONS[attempt.reason] ??
                REASONS[attempt.reason] ??
                'Author setup unavailable; Wishpool covers this step.'}
            </p>
            {attempt.admitted_at ? <DateText iso={attempt.admitted_at} /> : null}
            {attempt.finished_at ? (
              <>
                {' '}
                · Finished <DateText iso={attempt.finished_at} />
              </>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  );
}
