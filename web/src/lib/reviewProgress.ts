import type { FeedbackLetter, RefereeFile, RefereeRound, Submission } from '../api/types';

export const RECHECK_WAITING = 'Your response is saved. A follow-up review is waiting to start.';

export function hasWaitingResponse(file: RefereeFile | undefined, submission: Submission): boolean {
  const version = submission.versions.at(-1)?.number;
  return (
    file?.finding_responses?.some(
      (response) =>
        response.version === version &&
        response.claims_revision === submission.claims_revision &&
        !file.rounds.some(
          (round) =>
            round.version === version &&
            round.claims_revision === submission.claims_revision &&
            round.response_ids?.includes(response.id),
        ),
    ) ?? false
  );
}

export const REVIEW_STAGES = [
  ['referee', 'Referee report'],
  ['audit', 'Source audit'],
  ['advice', 'Improvement suggestions'],
  ['letter', 'Feedback letter'],
  ['formal', 'Lean check'],
] as const;

export function reviewDelivered(round: RefereeRound, letters: readonly FeedbackLetter[]) {
  return letters.some((letter) => letter.round === round.number);
}

/** Only a delivered letter establishes delivery; hidden drafts never do. */
export function reviewProgressText(
  round: RefereeRound,
  letters: readonly FeedbackLetter[],
  includeFormal = false,
) {
  if (!round.progress) return undefined;
  const delivered = reviewDelivered(round, letters);
  const stages = delivered
    ? includeFormal
      ? REVIEW_STAGES.slice(4)
      : []
    : REVIEW_STAGES.slice(0, 4);
  for (const [key, label] of stages) {
    const step = round.progress[key];
    if (step.state === 'failed')
      return `${label} ${key === 'advice' ? 'need' : 'needs'} attention. Your source and completed reports are saved.`;
    if (step.state === 'running')
      return `${label} ${key === 'advice' ? 'are' : 'is'} in progress. You can leave this page and return later.`;
    if (step.state === 'pending')
      return `${label} ${key === 'advice' ? 'are' : 'is'} waiting to start. You can leave this page and return later.`;
  }
  return delivered
    ? 'Your feedback letter is ready below.'
    : 'The review stages have finished. No feedback letter has been delivered for this round.';
}
