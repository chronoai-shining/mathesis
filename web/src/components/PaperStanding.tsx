import type { FeedbackLetter, RefereeRound, RejectReason, Submission } from '../api/types';
import { RECHECK_WAITING, reviewDelivered, reviewProgressText } from '../lib/reviewProgress';
import { rejectReasonText } from '../lib/labels';

export function publicationReason(reason: RejectReason): string {
  switch (reason.reason) {
    case 'known_result':
      return `Statement ${reason.claim} already follows from ${reason.prior.locator}.`;
    case 'bind_only':
      return 'No checked main result carries new mathematical content.';
    case 'no_main_result':
      return 'The paper has no proved main result.';
    case 'incorrect_result':
    case 'unchecked_result':
      return rejectReasonText(reason);
    default:
      return reason.detail;
  }
}

const STEPS = ['Referee', 'Check', 'Decision', 'Letter'] as const;

/** Which of the four review steps is under way (0-based); 4 once the letter is out. */
function progressStep(round: RefereeRound | undefined): number {
  if (round?.progress) {
    const index = (['referee', 'audit', 'advice', 'letter'] as const).findIndex(
      (key) => !['done', 'skipped'].includes(round.progress![key].state),
    );
    return index < 0 ? 4 : index;
  }
  if (!round || round.referee.state.state !== 'done') return 0;
  if (round.audit?.state.state !== 'done') return 1;
  if (round.letter?.state.state !== 'done') return 3;
  return 4;
}

const PROGRESS_TEXT = [
  'The referee is reading your paper.',
  'The referee’s report is being checked against your source.',
  'The decision is being made.',
  'The letter is being written.',
];

/** Publication status is decided by the escape analysis; recommendations are feedback. */
export function PaperStanding({
  submission,
  round,
  letters = [],
  awaitingRecheck = false,
  onRevise,
}: {
  submission: Submission;
  round?: RefereeRound;
  letters?: readonly FeedbackLetter[];
  awaitingRecheck?: boolean;
  onRevise?: () => void;
}) {
  const status = submission.status;
  if (status.state === 'draft') return null;
  const audit = round?.audit?.state.state === 'done' ? round.audit.state.result : undefined;
  const probe = round?.formal?.state.state === 'done' ? round.formal.state.result : undefined;
  const failed = round?.progress
    ? (['referee', 'audit', 'advice', 'letter'] as const).some(
        (key) => round.progress![key].state === 'failed',
      )
    : [round?.referee, round?.audit].some((s) => s?.state.state === 'failed');
  const delivered =
    round !== undefined &&
    (round.progress
      ? reviewDelivered(round, letters)
      : round.letter?.state.state === 'done' ||
        letters.some((letter) => letter.round === round.number));
  const progressText = round ? reviewProgressText(round, letters) : undefined;
  const privatePending = status.state === 'review_complete' && round !== undefined && !delivered;
  const headline =
    status.state === 'review_complete'
      ? !round
        ? 'Private Orchestra review'
        : awaitingRecheck
          ? 'Your response is awaiting review'
          : privatePending
            ? failed
              ? 'Your review needs attention'
              : audit
                ? 'Your source audit is ready'
                : round.response_ids?.length
                  ? 'Your response is being reviewed'
                  : 'Your private review is in progress'
            : 'Your private review is ready'
      : status.state === 'accepted'
        ? submission.kind === 'conjecture'
          ? 'Displayed'
          : 'Accepted'
        : status.state === 'not_accepted'
          ? submission.kind === 'conjecture'
            ? 'Not displayed'
            : 'Not accepted'
          : status.state === 'withdrawn'
            ? 'Withdrawn'
            : 'Under review';
  const tone =
    status.state === 'accepted'
      ? 'good'
      : status.state === 'not_accepted'
        ? 'bad'
        : status.state === 'withdrawn'
          ? 'muted'
          : 'accent';
  const reasons =
    submission.decision?.decision === 'not_accepted' ? submission.decision.reasons : [];
  const revisions =
    audit?.verdict === 'minor_revision'
      ? 'minor'
      : audit?.verdict === 'major_revision'
        ? 'major'
        : undefined;
  const mayRevise =
    status.state === 'review_complete' ||
    status.state === 'not_accepted' ||
    (status.state === 'accepted' && revisions !== undefined);
  const step = progressStep(round);
  const facts = audit
    ? [
        { n: audit.claims.length, label: 'statements' },
        { n: audit.claims.filter((c) => c.correctness === 'correct').length, label: 'correct' },
        ...(probe
          ? [
              {
                n: probe.attempts.filter((a) => a.outcome === 'compiled').length,
                label: 'proved in Lean',
              },
            ]
          : []),
      ]
    : [];

  return (
    <section className={`standing standing-${tone}`} aria-label="Paper standing">
      <div className="standing-head">
        <h2 className="standing-title">{headline}</h2>
        {status.state === 'accepted' ? (
          <span className="standing-record">{status.record}</span>
        ) : null}
      </div>

      {awaitingRecheck ? (
        <p role="status">{RECHECK_WAITING}</p>
      ) : status.state === 'in_review' ||
        privatePending ||
        (round?.progress && !delivered && status.state !== 'withdrawn') ? (
        failed ? (
          <p role="status">
            {progressText ?? 'The review could not finish. Please try again later.'}
          </p>
        ) : (
          <>
            <ol className="standing-steps" aria-label="Review progress">
              {(round?.progress
                ? ['Referee', 'Check', 'Suggestions', 'Letter']
                : submission.purpose === 'review_only'
                  ? ['Referee', 'Check', 'Feedback', 'Letter']
                  : STEPS
              ).map((name, i) => (
                <li
                  key={name}
                  className={i < step ? 'is-done' : i === step ? 'is-current' : undefined}
                  aria-current={i === step ? 'step' : undefined}
                >
                  {name}
                </li>
              ))}
            </ol>
            <p role="status" className="standing-summary">
              {progressText ??
                (step === 3 && !round?.letter
                  ? 'The source audit is complete. The feedback letter has not been delivered yet.'
                  : PROGRESS_TEXT[Math.min(step, PROGRESS_TEXT.length - 1)])}
            </p>
          </>
        )
      ) : null}

      {status.state === 'not_accepted' && reasons.length > 0 ? (
        <ul className="standing-reasons">
          {reasons.map((r, i) => (
            <li key={i}>{publicationReason(r)}</li>
          ))}
        </ul>
      ) : null}
      {(status.state === 'accepted' ||
        status.state === 'not_accepted' ||
        status.state === 'review_complete') &&
      audit ? (
        <p className="standing-summary">{audit.summary}</p>
      ) : null}
      {facts.length > 0 && status.state !== 'in_review' ? (
        <ul className="standing-facts">
          {facts.map((f) => (
            <li key={f.label}>
              <strong>{f.n}</strong> {f.label}
            </li>
          ))}
        </ul>
      ) : null}

      {(status.state === 'accepted' && revisions) || (mayRevise && onRevise) ? (
        <div className="standing-next">
          {status.state === 'accepted' && revisions ? (
            <p>The referee suggests {revisions} revisions — see the letter.</p>
          ) : (
            <p>You can revise the paper and submit a new version.</p>
          )}
          {mayRevise && onRevise ? (
            <button
              type="button"
              className="button button-small"
              aria-controls="standing-revision-form"
              onClick={onRevise}
            >
              Upload a revised version
            </button>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
