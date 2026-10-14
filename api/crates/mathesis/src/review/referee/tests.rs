use std::{
    collections::{BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use mathesis_core::{
    app::{App, Upload},
    memory::MemoryStores,
    model::{
        AiDisclosure, AiUse, Caller, ClaimConfirmation, FeedbackLetter, NewLetter, NewPaper,
        Recommendation, ReviewerIdentity, Role, StepState, Submission,
    },
    policy::Policy,
    ports::{JobKind, JobLease, SystemClock},
};
use mathesis_review::{
    ReviewResult,
    advisor::{Advisor, AdvisorInput},
    lean::{CheckedFile, FormalInput, FormalOut, Formalizer},
    oracle::{Oracle, OracleRequest, OracleStatus, OracleSubmitted},
    referee_prompts::{AdviceOut, AuditOut, AuditedClaimOut, FormalizationOut, LetterOut},
};
use tokio::sync::Mutex;

use super::*;
use crate::{
    latex::{Compile, LatexReader},
    review::MemoryJobs,
};

const SOURCE: &str = r"\documentclass{article}
\newtheorem{theorem}{Theorem}
\newtheorem{conjecture}{Conjecture}
\title{A paper}\author{A. Author}
\begin{document}
\begin{theorem}Every gap is finite.\end{theorem}
\begin{proof}By compactness.\end{proof}
\begin{conjecture}Every gap is small.\end{conjecture}
\end{document}";

struct FakeOracle {
    submitted: AtomicUsize,
    references: Mutex<Vec<String>>,
    outcomes: Mutex<VecDeque<ReviewResult<OracleStatus>>>,
    submit_error: AtomicUsize,
}

#[async_trait]
impl Oracle for FakeOracle {
    fn engine(&self) -> &str {
        "nyxid-oracle"
    }
    async fn submit(&self, request: &OracleRequest) -> ReviewResult<OracleSubmitted> {
        self.submitted.fetch_add(1, Ordering::SeqCst);
        self.references
            .lock()
            .await
            .push(request.client_ref.clone());
        if request.client_ref.starts_with("wishpool:problem:") {
            assert!(request.prompt.contains("C2") && request.pdf.is_none());
        } else {
            assert!(request.prompt.contains("C1") && request.prompt.contains("C2"));
            assert!(request.pdf.is_some());
        }
        assert!(request.prompt.contains("\"proved\": false"));
        if self.submit_error.swap(0, Ordering::SeqCst) != 0 {
            return Err(ReviewError::Transport("uncertain delivery".into()));
        }
        Ok(OracleSubmitted {
            task: "task-1".into(),
            queue_position: Some(2),
        })
    }
    async fn poll(&self, task: &str) -> ReviewResult<OracleStatus> {
        assert_eq!(task, "task-1");
        self.outcomes
            .lock()
            .await
            .pop_front()
            .expect("unexpected poll")
    }
}

#[derive(Default)]
struct FakeAdvisor {
    advice: AtomicUsize,
    letters: AtomicUsize,
    fail_advice: bool,
    fail_letter: bool,
    /// Propose C1 and the open C2 for formalization.
    propose: AtomicBool,
    audits: AtomicUsize,
    known: bool,
    followup_concern: bool,
    audit_delay: Duration,
    audit_failures: AtomicUsize,
    reclaim_during_audit: Option<Arc<MemoryJobs>>,
}

#[derive(Default)]
struct FakeFormalizer {
    calls: AtomicUsize,
    never_completes: bool,
    corrections: Mutex<Vec<Option<String>>>,
}

#[async_trait]
impl Formalizer for FakeFormalizer {
    fn engine(&self) -> &str {
        "test-lean"
    }
    fn model(&self) -> &str {
        "codex-test"
    }
    async fn formalize(
        &self,
        input: &FormalInput,
        work: &std::path::Path,
    ) -> ReviewResult<FormalOut> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.never_completes {
            return std::future::pending().await;
        }
        if input.conjecture {
            self.corrections.lock().await.push(input.correction.clone());
            let suffix = if input.correction.is_some() {
                "∀ n : Nat, n + 0 = n"
            } else {
                "∀ n : Nat, n = n"
            };
            return Ok(FormalOut {
                toolchain: "Lean fake checked workspace".into(),
                files: input
                    .targets
                    .iter()
                    .map(|t| CheckedFile {
                        claim: t.claim.clone(),
                        theorem: Some("wishpool_target_prop".into()),
                        lean: format!(
                            "import Mathlib\ndef wishpool_target_prop : Prop := {suffix}\n"
                        ),
                        compiled: true,
                        axioms: vec![],
                        note: "Every natural number equals itself.".into(),
                        log: String::new(),
                    })
                    .collect(),
                summary: "Statement elaborated".into(),
            });
        }
        assert!(
            input
                .source_dir
                .as_ref()
                .unwrap()
                .join("paper.tex")
                .exists()
        );
        assert!(!work.exists() || work.read_dir().unwrap().next().is_none());
        let targets: Vec<&str> = input.targets.iter().map(|t| t.claim.as_str()).collect();
        assert_eq!(targets, ["C1"]);
        Ok(FormalOut {
            toolchain: "leanprover/lean4:v4.33.0, Mathlib v4.33.0".into(),
            files: vec![
                CheckedFile {
                    claim: "C1".into(),
                    theorem: Some("Mathesis.C1.main".into()),
                    lean: "import Mathlib".into(),
                    compiled: true,
                    axioms: vec!["propext".into()],
                    note: "Faithful".into(),
                    log: String::new(),
                },
                CheckedFile {
                    claim: "C2".into(),
                    theorem: None,
                    lean: String::new(),
                    compiled: true,
                    axioms: vec![],
                    note: "not a target".into(),
                    log: String::new(),
                },
            ],
            summary: "C1 formalized".into(),
        })
    }
}

#[async_trait]
impl Advisor for FakeAdvisor {
    async fn inspect_with_store(
        &self,
        _: &AdvisorInput,
        _: &str,
        _: &dyn mathesis_review::cma::RunStore,
    ) -> ReviewResult<mathesis_review::advisor::InspectionOut> {
        Ok(mathesis_review::advisor::InspectionOut {
            verdict: "faithful".into(),
            reason: "Definitions and quantifiers agree.".into(),
            ..Default::default()
        })
    }

    fn engine(&self) -> &str {
        "test-advisor"
    }
    fn model(&self) -> &str {
        "codex-test"
    }
    async fn audit(&self, input: &AdvisorInput) -> ReviewResult<AuditOut> {
        self.audits.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(self.audit_delay).await;
        if self
            .audit_failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(ReviewError::Transport("advisor CLI timed out".into()));
        }
        if let Some(jobs) = &self.reclaim_during_audit {
            // Model completion after a replacement worker obtained the lease.
            tokio::time::sleep(Duration::from_secs(31 * 60)).await;
            jobs.claim().await.unwrap().expect("replacement lease");
        }
        assert!(
            input
                .source_dir
                .as_ref()
                .unwrap()
                .join("paper.tex")
                .exists()
        );
        assert!(!input.referee.text.is_empty());
        Ok(AuditOut {
            concerns: if self.followup_concern && input.followup.is_none() {
                vec![mathesis_review::referee_prompts::AuditedConcernOut {
                    concern: "The domain of the compactness argument needs clarification.".into(),
                    note: "Clarify the boundary convention.".into(),
                    status: "confirmed".into(), basis: "offline".into(),
                    evidence: "The displayed hypotheses do not specify the boundary convention.".into(),
                    ..Default::default()
                }]
            } else { vec![] },
            response_assessments: input.followup.as_ref().map(|f| f.response_ids.iter().map(|id|
                mathesis_review::referee_prompts::ResponseAssessmentOut {
                    response_id: id.clone(), status: "addressed".into(),
                    explanation: "The author's stated convention matches the quantified domain in the source.".into(),
                    evidence_paths: vec![],
                }).collect()).unwrap_or_default(),
            verdict: "major_revision".into(),
            summary: "The argument is correct and carries new content.".into(),
            claims: input
                .statements
                .iter()
                .map(|c| AuditedClaimOut {
                    claim: c.id.clone(),
                    conjecture: (input.kind == "conjecture" && !c.proved).then(|| {
                        mathesis_review::referee_prompts::ConjectureOut {
                            well_posed: Some(true),
                            well_posed_reason: "Defined and quantified".into(),
                            status: "open".into(),
                            status_reason: "Open in available source/report".into(),
                            escape: "content".into(),
                            escape_reason: "Needs a new bound".into(),
                            ..Default::default()
                        }
                    }),
                    correctness: if c.proved { "correct" } else { "not_checked" }.into(),
                    comment: "The source supplies a compactness lemma.".into(),
                    shape: c.proved.then(|| "content".into()),
                    witnesses: if c.proved {
                        vec!["A compactness lemma".into()]
                    } else {
                        vec![]
                    },
                    known: (self.known && c.proved).then(|| {
                        "A reported source [opened: https://example.test/reported-work]".into()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })
    }
    async fn advise(&self, input: &AdvisorInput) -> ReviewResult<AdviceOut> {
        assert!(input.audit.is_some() && input.decision.is_some());
        self.advice.fetch_add(1, Ordering::SeqCst);
        if self.fail_advice {
            return Err(ReviewError::Transport("advisor deadline".into()));
        }
        assert!(
            input
                .source_dir
                .as_ref()
                .unwrap()
                .join("paper.tex")
                .exists()
        );
        assert_eq!(input.main_file.as_deref(), Some("paper.tex"));
        assert!(!input.referee.text.is_empty());
        assert!(input.text.as_ref().unwrap().contains("compactness"));
        let formalization = if self.propose.load(Ordering::SeqCst) {
            ["C1", "C2"]
                .into_iter()
                .map(|claim| FormalizationOut {
                    claim: claim.into(),
                    feasibility: "ready".into(),
                    effort: "small".into(),
                    plan: "state and prove".into(),
                    ..Default::default()
                })
                .collect()
        } else {
            vec![]
        };
        Ok(AdviceOut {
            summary: "A useful argument".into(),
            formalization,
            ..Default::default()
        })
    }
    async fn draft_letter(
        &self,
        input: &AdvisorInput,
        advice: Option<&AdviceOut>,
    ) -> ReviewResult<LetterOut> {
        assert!(
            input.audit.is_some() && input.decision.as_ref().unwrap().get("decision").is_some()
        );
        self.letters.fetch_add(1, Ordering::SeqCst);
        if self.fail_letter {
            return Err(ReviewError::Output("unusable letter".into()));
        }
        if let Some(advice) = advice {
            assert!(advice.summary.starts_with("A useful argument"));
        }
        Ok(LetterOut {
            subject: "Feedback".into(),
            body: "A precise mathematical point.".into(),
            note: "An argument.".into(),
        })
    }
}

struct World {
    worker: Worker,
    jobs: Arc<MemoryJobs>,
    stores: Arc<MemoryStores>,
    paper: Submission,
    author: Caller,
    oracle: Arc<FakeOracle>,
    advisor: Arc<FakeAdvisor>,
    formalizer: Arc<FakeFormalizer>,
    _work: tempfile::TempDir,
}

impl World {
    async fn new(outcomes: Vec<ReviewResult<OracleStatus>>, pdf: bool) -> Self {
        Self::new_kind(outcomes, pdf, mathesis_core::model::SubmissionKind::Paper).await
    }
    async fn new_kind(
        outcomes: Vec<ReviewResult<OracleStatus>>,
        pdf: bool,
        kind: mathesis_core::model::SubmissionKind,
    ) -> Self {
        Self::new_kind_with_cma(outcomes, pdf, kind, false).await
    }
    async fn new_kind_with_cma(
        outcomes: Vec<ReviewResult<OracleStatus>>,
        pdf: bool,
        kind: mathesis_core::model::SubmissionKind,
        cma_enabled: bool,
    ) -> Self {
        let stores = Arc::new(MemoryStores::default());
        let mut ports = stores.ports(Arc::new(SystemClock), Arc::new(LatexReader));
        if cma_enabled {
            ports.cma_setup = Arc::new(TestCmaSetup);
        }
        let app = App::new(ports, Policy::default(), BTreeSet::new());
        let author = app
            .ensure_service_account(&"author".into(), "Author", Role::Endorser)
            .await
            .unwrap();
        let reviewer = app
            .ensure_service_account(&"claude".into(), "Review engine", Role::Reviewer)
            .await
            .unwrap();
        let referee_account = app
            .ensure_service_account(&"wishpool:referee".into(), "Referee", Role::Reviewer)
            .await
            .unwrap();
        let paper = app
            .submit_paper(
                &author,
                NewPaper {
                    purpose: Default::default(),
                    review_scope: Default::default(),
                    processing_consent: Some("ai-review-2026-10-11".into()),
                    kind,
                    make_public_after_acceptance: true,
                    typed_conjecture: None,
                    ai_disclosure: AiDisclosure {
                        level: AiUse::None,
                        statement: "No AI used in this paper.".into(),
                    },
                    authors: vec![],
                    msc: vec![],
                    doi: None,
                    open_to_contributors: false,
                },
                Upload {
                    filename: "paper.tex".into(),
                    bytes: SOURCE.as_bytes().to_vec(),
                },
            )
            .await
            .unwrap();
        if pdf {
            app.record_compilation(&reviewer, &paper.id, 1, Ok(b"%PDF".to_vec()))
                .await
                .unwrap();
        }
        let paper = app.submission(&author, &paper.id).await.unwrap();
        let claims = paper
            .extracted
            .iter()
            .map(|c| ClaimConfirmation {
                depends_on_conjectures: vec![],
                id: c.id.clone(),
                kind: c.kind,
                role: c.role,
                depends_on: vec![],
                settles: None,
                excluded: false,
            })
            .collect();
        let paper = app
            .confirm_claims(&author, &paper.id, claims)
            .await
            .unwrap();
        while stores.take_job().await.is_some() {}
        let jobs = Arc::new(MemoryJobs::new(stores.clone()));
        let oracle = Arc::new(FakeOracle {
            submitted: AtomicUsize::new(0),
            references: Mutex::new(vec![]),
            outcomes: Mutex::new(outcomes.into()),
            submit_error: AtomicUsize::new(0),
        });
        let advisor = Arc::new(FakeAdvisor::default());
        let formalizer = Arc::new(FakeFormalizer::default());
        let work = tempfile::tempdir().unwrap();
        let auditor_account = app
            .ensure_service_account(&"wishpool:auditor".into(), "Auditor", Role::Reviewer)
            .await
            .unwrap();
        let worker = Worker {
            cma_factory: None,
            app,
            jobs: jobs.clone(),
            compile: Compile {
                tex_bin: "unused".into(),
                cache_dir: work.path().into(),
                texmf_home: None,
                timeout: Duration::from_secs(1),
            },
            model: None,
            reviewer,
            auditor_account,
            referee_account,
            referee_model: "chatgpt-pro".into(),
            oracle: Some(oracle.clone()),
            advisor: Some(advisor.clone()),
            formalizer: Some(formalizer.clone()),
            oracle_poll: Duration::ZERO,
            advisor_work_dir: work.path().into(),
            audit_budget: Duration::from_secs(3900),
            formal_budget: Duration::from_secs(1500),
        };
        Self {
            worker,
            jobs,
            stores,
            paper,
            author,
            oracle,
            advisor,
            formalizer,
            _work: work,
        }
    }

    async fn job(&self) -> LeasedJob {
        if let Some(job) = self.jobs.current(&self.paper.id, JobKind::Referee).await {
            return job;
        }
        mathesis_core::ports::ReviewQueue::enqueue(&*self.stores, &self.paper.id, JobKind::Referee)
            .await
            .unwrap();
        self.jobs.claim().await.unwrap().unwrap()
    }

    async fn claim_referee(&self) -> LeasedJob {
        loop {
            let job = self.jobs.claim().await.unwrap().expect("referee queued");
            if job.kind == JobKind::Referee {
                return job;
            }
            self.jobs
                .defer(&job, Duration::from_secs(24 * 3600))
                .await
                .unwrap();
        }
    }

    async fn file(&self) -> mathesis_core::model::RefereeFile {
        mathesis_core::ports::RefereeStore::get(&*self.stores, &self.paper.id)
            .await
            .unwrap()
            .unwrap_or_else(|| mathesis_core::model::RefereeFile::new(self.paper.id.clone()))
    }

    async fn run_round(&self) {
        let mut job = self.job().await;
        for _ in 0..12 {
            match self.worker.referee_job(&job).await {
                Ok(false) => return,
                Ok(true) => {
                    job = self
                        .worker
                        .jobs
                        .claim()
                        .await
                        .unwrap()
                        .expect("deferred job")
                }
                Err(_) => panic!("worker failed"),
            }
        }
        panic!("round did not settle");
    }
}

fn completed(recommendation: &str) -> ReviewResult<OracleStatus> {
    Ok(OracleStatus::Completed { model: None, text: serde_json::json!({"recommendation":recommendation,"summary":"A review",
        "claims":[{"claim":"C1","shape":"content","witnesses":["A compactness lemma"],"known":"A reported source [opened: https://example.test/reported-work]","note":"The intermediate argument"},
            {"claim":"C2","shape":"bind_only"}], "limits":["Computation was not reproduced"]}).to_string() })
}

#[tokio::test]
async fn polling_survives_transport_errors_without_resubmitting_or_burning_attempts() {
    let w = World::new(
        vec![
            Err(ReviewError::Transport("temporary".into())),
            Ok(OracleStatus::Queued { position: Some(1) }),
            Ok(OracleStatus::Running),
            completed("accept"),
        ],
        true,
    )
    .await;
    w.run_round().await;
    let file = w.file().await;
    let round = file.current().unwrap();
    assert!(round.is_settled());
    assert_eq!(round.referee.attempts, 1);
    assert_eq!(round.referee.input_digest.as_ref().unwrap().len(), 64);
    assert_eq!(
        round.referee.client_ref.as_deref(),
        Some(format!("wishpool:{}:r1:v1:c1", w.paper.id).as_str())
    );
    assert_eq!(
        round.referee.done().unwrap().recommendation,
        Some(Recommendation::Accept)
    );
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 1);
    assert_eq!(w.advisor.advice.load(Ordering::SeqCst), 1);
    assert_eq!(w.advisor.letters.load(Ordering::SeqCst), 1);
    assert!(round.advice.input_digest.is_some() && round.letter.input_digest.is_some());
    let judgements = w
        .worker
        .app
        .judgements(&w.worker.referee_account, &w.paper.id)
        .await
        .unwrap();
    assert_eq!(judgements.len(), 1);
    assert!(
        matches!(&judgements[0].reviewer, ReviewerIdentity::Machine { account, engine, model } if account.as_str() == "wishpool:referee" && engine == "nyxid-oracle" && model.as_deref() == Some("chatgpt-pro"))
    );
    assert!(
        judgements[0]
            .rationale
            .contains("A reported source [opened: https://example.test/reported-work]")
    );
    assert!(
        w.worker
            .app
            .referee(&w.author, &w.paper.id)
            .await
            .unwrap()
            .letters
            .len()
            == 1
    );
    assert!(!w.worker.referee_job(&w.job().await).await.unwrap());
    assert_eq!(
        w.worker
            .app
            .judgements(&w.worker.referee_account, &w.paper.id)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn uncertain_submit_reuses_persisted_reference() {
    let w = World::new(vec![completed("minor_revision")], true).await;
    w.oracle.submit_error.store(1, Ordering::SeqCst);
    assert!(w.worker.referee_job(&w.job().await).await.unwrap());
    let step = w.file().await.current().unwrap().referee.clone();
    assert!(matches!(step.state, StepState::Pending));
    assert!(step.client_ref.is_some() && step.input_digest.is_some());
    let job = w.worker.jobs.claim().await.unwrap().unwrap();
    assert_eq!(job.attempts, 1);
    assert!(w.worker.referee_job(&job).await.unwrap());
    w.run_round().await;
    let references = w.oracle.references.lock().await;
    assert_eq!(references.len(), 2);
    assert_eq!(references[0], references[1]);
}

#[tokio::test]
async fn accepted_round_sends_letter_before_formalizing_proved_statements() {
    let w = World::new(vec![completed("minor_revision")], true).await;
    w.advisor.propose.store(true, Ordering::SeqCst);
    w.run_round().await;
    let file = w.file().await;
    let round = file.current().unwrap();
    let probe = round.formal.done().expect("probe recorded");
    assert_eq!(w.formalizer.calls.load(Ordering::SeqCst), 1);
    assert_eq!(round.formal.engine.as_deref(), Some("test-lean"));
    assert_eq!(probe.attempts.len(), 1);
    assert_eq!(probe.attempts[0].claim.as_str(), "C1");
    assert_eq!(
        probe.attempts[0].outcome,
        mathesis_core::model::ProbeOutcome::Compiled
    );
    let StepState::Done { at: letter_at, .. } = round.letter.state else {
        panic!()
    };
    let StepState::Done { at: formal_at, .. } = round.formal.state else {
        panic!()
    };
    assert!(letter_at <= formal_at);
    assert_eq!(file.letters.len(), 1);
    assert_eq!(file.letters[0].sent_by.as_str(), "wishpool:auditor");
    assert_eq!(
        file.letters[0].assessment,
        Some(Recommendation::MajorRevision)
    );
    assert!(!file.letters[0].edited);
    assert!(round.letter.done().is_some());
}

#[tokio::test(start_paused = true)]
async fn formalization_budget_failure_follows_delivered_letter_and_defers_job() {
    let mut w = World::new(vec![completed("accept")], true).await;
    w.advisor.propose.store(true, Ordering::SeqCst);
    let formalizer = Arc::new(FakeFormalizer {
        never_completes: true,
        ..Default::default()
    });
    w.worker.formalizer = Some(formalizer.clone());
    w.worker.formal_budget = Duration::from_secs(3600 + 300);
    let mut job = w.job().await;
    // Submit, report, audit, decision + advice, then deliver letter.
    for _ in 0..5 {
        assert!(w.worker.referee_job(&job).await.unwrap());
        job = w.claim_referee().await;
    }
    let started = tokio::time::Instant::now();
    w.worker.handle(job.clone()).await;
    // A one-hour Codex deadline plus discovery/check margin runs beyond the
    // original lease, which the heartbeat must keep alive until deferral.
    assert_eq!(started.elapsed(), Duration::from_secs(3900));
    let file = w.file().await;
    let round = file.current().unwrap();
    assert!(matches!(
        &round.formal.state,
        StepState::Failed {
            reason,
            detail: None,
            retryable: false,
            ..
        } if reason == "the formalization step exceeded its time budget"
    ));
    assert_eq!(round.formal.attempts, 1);
    assert!(round.letter.done().is_some());
    assert_eq!(w.advisor.letters.load(Ordering::SeqCst), 1);

    let deferred = w.claim_referee().await;
    assert_eq!(deferred.submission, job.submission);
    assert_eq!(deferred.kind, job.kind);
    assert_eq!(deferred.attempts, job.attempts);
    assert_ne!(deferred.lease, job.lease);
    assert!(!w.worker.referee_job(&deferred).await.unwrap());
    let file = w.file().await;
    let round = file.current().unwrap();
    assert!(round.is_settled());
    assert!(round.letter.done().is_some());
    assert_eq!(w.advisor.letters.load(Ordering::SeqCst), 1);
    assert!(!w.worker.referee_job(&deferred).await.unwrap());
    assert_eq!(formalizer.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn negative_recommendation_still_gets_decision_advice_and_letter() {
    let w = World::new(vec![completed("major_revision")], true).await;
    w.run_round().await;
    let file = w.file().await;
    let round = file.current().unwrap();
    assert!(round.advice.done().is_some());
    assert!(matches!(round.formal.state, StepState::Skipped { .. }));
    assert!(round.letter.done().is_some());
    assert_eq!(w.advisor.advice.load(Ordering::SeqCst), 1);
    assert_eq!(w.formalizer.calls.load(Ordering::SeqCst), 0);
    assert_eq!(w.advisor.letters.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn confirmation_before_compilation_waits_without_submitting_or_burning_attempts() {
    let w = World::new(vec![completed("accept")], false).await;
    let mut job = w.job().await;
    for _ in 0..3 {
        assert!(w.worker.referee_job(&job).await.unwrap());
        let step = w.file().await.current().unwrap().referee.clone();
        assert!(matches!(step.state, StepState::Pending));
        assert_eq!(step.attempts, 0);
        assert!(step.client_ref.is_none() && step.input_digest.is_none());
        assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 0);
        job = w.worker.jobs.claim().await.unwrap().unwrap();
        assert_eq!(job.attempts, 1);
    }
    w.worker
        .app
        .record_compilation(&w.worker.reviewer, &w.paper.id, 1, Ok(b"%PDF".to_vec()))
        .await
        .unwrap();
    assert!(w.worker.referee_job(&job).await.unwrap());
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 1);
    assert!(matches!(
        w.file().await.current().unwrap().referee.state,
        StepState::Running { .. }
    ));
    w.run_round().await;
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 1);
    assert!(w.file().await.current().unwrap().is_settled());
}

#[tokio::test]
async fn failed_compilation_settles_without_oracle_or_advisor_work() {
    let w = World::new(vec![], false).await;
    w.worker
        .app
        .record_compilation(&w.worker.reviewer, &w.paper.id, 1, Err("TeX error".into()))
        .await
        .unwrap();
    w.run_round().await;
    let file = w.file().await;
    let round = file.current().unwrap();
    assert!(
        matches!(&round.referee.state, StepState::Failed { reason, retryable: false, .. } if reason == "the paper did not compile")
    );
    assert_eq!(round.referee.attempts, 0);
    assert!(round.is_settled());
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 0);
    assert_eq!(w.advisor.advice.load(Ordering::SeqCst), 0);
    assert_eq!(w.advisor.letters.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn remote_failure_and_missing_pdf_settle_without_advisor_work() {
    for status in [
        OracleStatus::Failed {
            reason: "prompt_delivery_uncertain".into(),
            detail: Some("page_crashed@waiting_response".into()),
        },
        OracleStatus::Cancelled,
    ] {
        let w = World::new(vec![Ok(status.clone())], true).await;
        w.run_round().await;
        let file = w.file().await;
        let round = file.current().unwrap();
        assert!(matches!(round.referee.state, StepState::Failed { .. }));
        if matches!(status, OracleStatus::Failed { .. }) {
            assert!(matches!(
                &round.referee.state,
                StepState::Failed {
                    retryable: true,
                    detail: Some(_),
                    ..
                }
            ));
        }
        assert!(matches!(round.advice.state, StepState::Skipped { .. }));
        assert!(matches!(round.letter.state, StepState::Skipped { .. }));
        assert_eq!(w.advisor.letters.load(Ordering::SeqCst), 0);
    }
    let w = World::new(vec![], true).await;
    // The PDF disappears after the submission snapshot was read.
    w.worker
        .app
        .record_compilation(&w.worker.reviewer, &w.paper.id, 1, Err("TeX error".into()))
        .await
        .unwrap();
    assert!(
        w.worker
            .resume_referee(&w.job().await, &w.paper)
            .await
            .unwrap()
    );
    w.run_round().await;
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 0);
    assert!(
        matches!(&w.file().await.current().unwrap().referee.state, StepState::Failed { reason, retryable: false, .. } if reason == "the paper has no compiled PDF")
    );
}

#[tokio::test]
async fn disabled_oracle_completes_without_starting_round() {
    let mut w = World::new(vec![], true).await;
    w.worker.oracle = None;
    assert!(!w.worker.referee_job(&w.job().await).await.unwrap());
    assert!(w.file().await.rounds.is_empty());
}

#[test]
fn digest_binds_boundaries_and_retryable_failure_reasons() {
    assert_ne!(input_digest("ab", b"c"), input_digest("a", b"bc"));
    for reason in [
        "infrastructure_retry_exhausted",
        "prompt_delivery_uncertain",
        "usage_limit_reached",
        "model_unavailable",
    ] {
        assert!(retryable(reason));
    }
    for reason in ["extraction_failure", "empty_response", "cancelled"] {
        assert!(!retryable(reason));
    }
}

#[tokio::test]
async fn existing_round_continues_after_editorial_decision() {
    let w = World::new(vec![completed("accept")], true).await;
    assert!(w.worker.referee_job(&w.job().await).await.unwrap());
    let job = w.worker.jobs.claim().await.unwrap().unwrap();
    let stores = w.worker.app.clone();
    // The public policy flow is independent of the in-flight referee round;
    // use an ordinary editor's out-of-scope report to produce non-acceptance.
    let editor = stores
        .ensure_service_account(&"editor".into(), "Editor", Role::Editor)
        .await
        .unwrap();
    stores
        .file_report(
            &editor,
            &w.paper.id,
            mathesis_core::model::Stage::Literature,
            mathesis_core::model::ReportDraft {
                outcome: mathesis_core::model::Outcome::Fail {
                    reason: mathesis_core::model::RejectReason::OutOfScope {
                        detail: "Outside the venue's field".into(),
                    },
                },
                summary: "Outside scope".into(),
                payload: mathesis_core::model::StagePayload::Literature {
                    prior: vec![],
                    searched: vec!["Paper source: scope assessment".into()],
                },
                evidence: vec![],
            },
            None,
        )
        .await
        .unwrap();
    stores.decide(&editor, &w.paper.id).await.unwrap();
    assert!(
        !stores
            .submission(&editor, &w.paper.id)
            .await
            .unwrap()
            .is_open()
    );
    assert!(w.worker.referee_job(&job).await.unwrap());
    w.run_round().await;
    assert!(w.file().await.current().unwrap().is_settled());
    assert_eq!(w.advisor.letters.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn referee_reading_corroborates_claude_without_changing_policy() {
    let w = World::new(vec![completed("accept")], true).await;
    let output =
        mapping::judgement(true, &["A compactness lemma".into()], "Claude reading").unwrap();
    w.worker
        .app
        .file_machine_judgement(
            &w.worker.reviewer,
            &w.paper.id,
            &"C1".into(),
            output,
            FiledBy {
                engine: "openai-compatible".into(),
                model: Some("claude-test".into()),
            },
        )
        .await
        .unwrap();
    w.run_round().await;
    let analysis = w.worker.app.analysis(&w.author, &w.paper.id).await.unwrap();
    let reading = analysis
        .claims
        .iter()
        .find(|c| c.claim.as_str() == "C1")
        .unwrap();
    assert_eq!(
        reading.judgement.as_ref().unwrap().standing,
        mathesis_core::judgement::Standing::Corroborated
    );
    let paper = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    assert!(matches!(paper.status, SubmissionStatus::Accepted { .. }));
    assert!(
        paper
            .latest_report(mathesis_core::model::Stage::Escape)
            .is_some()
    );
}

#[tokio::test]
async fn malformed_answer_is_retained_and_missing_advisor_is_explicit() {
    let mut w = World::new(
        vec![Ok(OracleStatus::Completed {
            text: "A readable but unstructured review".into(),
            model: None,
        })],
        true,
    )
    .await;
    w.worker.advisor = None;
    w.run_round().await;
    let file = w.file().await;
    let round = file.current().unwrap();
    let report = round.referee.done().unwrap();
    assert_eq!(report.text, "A readable but unstructured review");
    assert!(report.recommendation.is_none());
    assert!(!report.limits.is_empty());
    assert!(matches!(round.advice.state, StepState::Skipped { .. }));
    assert!(
        matches!(&round.audit.state, StepState::Failed { reason, .. } if reason == "no Codex auditor configured")
    );
    let mut w = World::new(vec![completed("accept")], true).await;
    w.worker.advisor = None;
    w.run_round().await;
    assert!(matches!(
        &w.file().await.current().unwrap().audit.state,
        StepState::Failed { reason, .. } if reason == "no Codex auditor configured"
    ));
}

#[tokio::test]
async fn advisor_failure_still_allows_letter_and_invalid_letter_settles_failed() {
    for fail_advice in [true, false] {
        let mut w = World::new(vec![completed("accept")], true).await;
        let advisor = Arc::new(FakeAdvisor {
            fail_advice,
            fail_letter: !fail_advice,
            ..Default::default()
        });
        w.worker.advisor = Some(advisor.clone());
        w.run_round().await;
        let file = w.file().await;
        let round = file.current().unwrap();
        assert!(round.is_settled());
        if fail_advice {
            assert!(matches!(
                round.advice.state,
                StepState::Failed {
                    retryable: true,
                    ..
                }
            ));
            assert!(round.letter.done().is_some());
        } else {
            assert!(round.advice.done().is_some());
            assert!(matches!(
                round.letter.state,
                StepState::Failed {
                    retryable: false,
                    ..
                }
            ));
        }
        assert_eq!(advisor.advice.load(Ordering::SeqCst), 1);
        assert_eq!(advisor.letters.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn observed_oracle_model_is_recorded_on_the_step_and_judgements() {
    let OracleStatus::Completed { text, .. } = completed("accept").unwrap() else {
        unreachable!();
    };
    let mut w = World::new(
        vec![Ok(OracleStatus::Completed {
            text,
            model: Some("gpt_6 · pro".into()),
        })],
        true,
    )
    .await;
    assert!(w.worker.referee_job(&w.job().await).await.unwrap());
    w.worker.referee_model = "gpt-new-label".into();
    w.run_round().await;
    assert_eq!(
        w.file().await.current().unwrap().referee.model.as_deref(),
        Some("gpt_6 · pro")
    );
    let judgements = w
        .worker
        .app
        .judgements(&w.worker.referee_account, &w.paper.id)
        .await
        .unwrap();
    assert_eq!(judgements.len(), 1);
    assert!(
        matches!(&judgements[0].reviewer, ReviewerIdentity::Machine { model, .. } if model.as_deref() == Some("gpt_6 · pro"))
    );
}

#[tokio::test]
async fn model_provenance_survives_config_changes_and_uncertain_admission() {
    let mut w = World::new(vec![completed("accept")], true).await;
    assert!(w.worker.referee_job(&w.job().await).await.unwrap());
    w.worker.referee_model = "gpt-new-label".into();
    let job = w.worker.jobs.claim().await.unwrap().unwrap();
    assert!(w.worker.referee_job(&job).await.unwrap());
    let judgements = w
        .worker
        .app
        .judgements(&w.worker.referee_account, &w.paper.id)
        .await
        .unwrap();
    assert!(
        matches!(&judgements[0].reviewer, ReviewerIdentity::Machine { model, .. } if model.as_deref() == Some("chatgpt-pro"))
    );
    let mut w = World::new(vec![], true).await;
    w.oracle.submit_error.store(1, Ordering::SeqCst);
    assert!(w.worker.referee_job(&w.job().await).await.unwrap());
    w.worker.referee_model = "gpt-new-label".into();
    w.run_round().await;
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 1);
    assert!(matches!(
        &w.file().await.current().unwrap().referee.state,
        StepState::Failed {
            retryable: false,
            ..
        }
    ));
}

#[tokio::test]
async fn sent_assessment_round_trips_without_becoming_a_publication_decision() {
    let w = World::new(vec![completed("accept")], true).await;
    w.run_round().await;
    let app = &w.worker.app;
    let editor = app
        .ensure_service_account(&"editor".into(), "Editor", Role::Editor)
        .await
        .unwrap();
    let before = app.submission(&w.author, &w.paper.id).await.unwrap();
    let decision = app.preview_decision(&editor, &w.paper.id).await.unwrap();
    let draft = w
        .file()
        .await
        .current()
        .unwrap()
        .letter
        .done()
        .unwrap()
        .clone();
    let mut request = serde_json::json!({
        "subject": draft.subject, "body": draft.body, "note": draft.note
    });
    let legacy: NewLetter = serde_json::from_value(request.clone()).unwrap();
    assert_eq!(legacy.assessment, None);
    let legacy = app
        .send_feedback(&editor, &w.paper.id, legacy)
        .await
        .unwrap();
    let legacy_json = serde_json::to_value(&legacy).unwrap();
    assert!(legacy_json.get("assessment").is_none());
    assert_eq!(
        serde_json::from_value::<FeedbackLetter>(legacy_json).unwrap(),
        legacy
    );

    // The editor can choose an assessment different from the referee's advice.
    request["assessment"] = serde_json::json!("major_revision");
    let letter = app
        .send_feedback(
            &editor,
            &w.paper.id,
            serde_json::from_value(request).unwrap(),
        )
        .await
        .unwrap();
    let serialized = serde_json::to_value(&letter).unwrap();
    assert_eq!(serialized["assessment"], "major_revision");
    assert_eq!(
        serde_json::from_value::<FeedbackLetter>(serialized).unwrap(),
        letter
    );
    let view = app.referee(&w.author, &w.paper.id).await.unwrap();
    assert_eq!(view.rounds.len(), 1);
    assert_eq!(&view.letters[1..], &[legacy, letter]);
    assert_eq!(
        app.submission(&w.author, &w.paper.id).await.unwrap(),
        before
    );
    assert_eq!(
        app.preview_decision(&editor, &w.paper.id).await.unwrap(),
        decision
    );
}

#[tokio::test]
async fn known_main_result_gets_advice_and_auto_letter_but_no_formal_probe() {
    let mut w = World::new(vec![completed("accept")], true).await;
    let advisor = Arc::new(FakeAdvisor {
        known: true,
        ..Default::default()
    });
    advisor.propose.store(true, Ordering::SeqCst);
    w.worker.advisor = Some(advisor.clone());
    w.run_round().await;
    let paper = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    assert_eq!(paper.status, SubmissionStatus::NotAccepted);
    assert_eq!(advisor.advice.load(Ordering::SeqCst), 1);
    assert_eq!(advisor.letters.load(Ordering::SeqCst), 1);
    assert_eq!(w.formalizer.calls.load(Ordering::SeqCst), 0);
    let file = w.file().await;
    assert!(
        file.letters[0]
            .body
            .starts_with("Your paper is not accepted.")
    );
    assert!(
        file.letters[0]
            .body
            .contains("A reported source [opened: https://example.test/reported-work]")
    );
    assert!(matches!(
        file.current().unwrap().formal.state,
        StepState::Skipped { .. }
    ));
    assert_eq!(
        file.letters[0].assessment,
        Some(Recommendation::MajorRevision)
    );
}

async fn prepare_audit(w: &World) -> LeasedJob {
    let mut job = w.job().await;
    for _ in 0..2 {
        assert!(w.worker.referee_job(&job).await.unwrap());
        job = w.worker.jobs.claim().await.unwrap().unwrap();
    }
    assert!(w.file().await.current().unwrap().referee.done().is_some());
    job
}

#[tokio::test(start_paused = true)]
async fn shutdown_releases_running_audit_without_waiting_for_lease_expiry() {
    let mut w = World::new(vec![completed("accept")], true).await;
    let job = prepare_audit(&w).await;
    w.worker.advisor = Some(Arc::new(FakeAdvisor {
        audit_delay: Duration::from_secs(41 * 60),
        ..Default::default()
    }));
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let started = tokio::time::Instant::now();
    let signal = async {
        tokio::time::sleep(Duration::from_secs(60)).await;
        shutdown_tx.send(true).unwrap();
    };
    tokio::join!(
        w.worker
            .handle_until_shutdown(job.clone(), &mut shutdown_rx),
        signal
    );
    assert_eq!(started.elapsed(), Duration::from_secs(60));
    let before = w.file().await;
    assert!(matches!(
        before.current().unwrap().audit.state,
        StepState::Running { .. }
    ));
    assert!(!w.jobs.renew(&job).await.unwrap());
    let replacement = w
        .jobs
        .claim()
        .await
        .unwrap()
        .expect("immediately resumable");
    assert_ne!(replacement.lease, job.lease);
    assert_eq!(
        replacement.attempts, job.attempts,
        "shutdown spends no retry"
    );
    w.worker.advisor = Some(w.advisor.clone());
    assert!(w.worker.referee_job(&replacement).await.unwrap());
    let after = w.file().await;
    assert!(after.current().unwrap().audit.done().is_some());
    assert_eq!(
        after.current().unwrap().referee,
        before.current().unwrap().referee
    );
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 1);
    // Cancellation cannot leave a late audit write or a detached heartbeat.
    let revision = after.revision;
    tokio::time::advance(Duration::from_secs(3600)).await;
    assert_eq!(w.file().await.revision, revision);
}

#[tokio::test(start_paused = true)]
async fn heartbeat_allows_audit_to_finish_past_the_original_lease() {
    let mut w = World::new(vec![completed("accept")], true).await;
    let advisor = Arc::new(FakeAdvisor {
        audit_delay: Duration::from_secs(41 * 60),
        ..Default::default()
    });
    w.worker.advisor = Some(advisor.clone());
    let job = prepare_audit(&w).await;
    let started = tokio::time::Instant::now();
    w.worker.handle(job.clone()).await;
    assert_eq!(started.elapsed(), Duration::from_secs(41 * 60));
    let file = w.file().await;
    let round = file.current().unwrap();
    assert!(round.audit.done().is_some());
    assert_eq!(round.audit.attempts, 1);
    assert!(matches!(round.advice.state, StepState::Pending));
    let next = w.worker.jobs.claim().await.unwrap().unwrap();
    assert_eq!(next.attempts, job.attempts);
    assert_ne!(next.lease, job.lease);
    assert_eq!(advisor.audits.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn audit_transport_timeout_retries_only_audit_with_existing_accounting() {
    let mut w = World::new(vec![completed("accept")], true).await;
    let advisor = Arc::new(FakeAdvisor {
        audit_failures: AtomicUsize::new(1),
        ..Default::default()
    });
    w.worker.advisor = Some(advisor.clone());
    let job = prepare_audit(&w).await;
    w.worker.handle(job).await;
    let file = w.file().await;
    let round = file.current().unwrap();
    assert!(matches!(round.audit.state, StepState::Running { .. }));
    assert_eq!(round.audit.attempts, 1);
    assert!(matches!(round.advice.state, StepState::Pending));
    assert!(matches!(round.letter.state, StepState::Pending));
    assert!(matches!(round.formal.state, StepState::Pending));
    assert_eq!(advisor.advice.load(Ordering::SeqCst), 0);
    assert!(w.worker.jobs.claim().await.unwrap().is_none());
    tokio::time::advance(Duration::from_secs(30)).await;
    let retry = w.worker.jobs.claim().await.unwrap().unwrap();
    assert_eq!(retry.attempts, 2);
    w.worker.handle(retry).await;
    let file = w.file().await;
    assert!(file.current().unwrap().audit.done().is_some());
    assert_eq!(file.current().unwrap().audit.attempts, 2);
    assert_eq!(file.current().unwrap().referee.attempts, 1);
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 1);
    assert_eq!(advisor.audits.load(Ordering::SeqCst), 2);
    assert_eq!(advisor.advice.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn stale_audit_result_cannot_overwrite_the_replacement_workers_step() {
    let mut w = World::new(vec![completed("accept")], true).await;
    let job = prepare_audit(&w).await;
    w.worker.advisor = Some(Arc::new(FakeAdvisor {
        reclaim_during_audit: Some(w.jobs.clone()),
        ..Default::default()
    }));
    assert!(matches!(
        w.worker.referee_job(&job).await,
        Err(Failure::LeaseLost)
    ));
    assert!(!w.jobs.renew(&job).await.unwrap());
    let replacement = w.jobs.current(&w.paper.id, JobKind::Referee).await.unwrap();
    assert_ne!(replacement.lease, job.lease);
    let before = w.file().await;
    assert!(matches!(
        before.current().unwrap().audit.state,
        StepState::Running { .. }
    ));
    w.worker.advisor = Some(w.advisor.clone());
    assert!(w.worker.referee_job(&replacement).await.unwrap());
    let after = w.file().await;
    assert!(after.current().unwrap().audit.done().is_some());
    let revision = after.revision;
    assert!(matches!(
        w.worker.referee_job(&job).await,
        Err(Failure::LeaseLost)
    ));
    assert_eq!(w.file().await.revision, revision);
}

struct LostHeartbeat {
    jobs: Arc<MemoryJobs>,
    lose_at: tokio::time::Instant,
    renewals: AtomicUsize,
    settlements: AtomicUsize,
}

#[async_trait]
impl JobLease for LostHeartbeat {
    async fn claim(&self) -> mathesis_core::CoreResult<Option<LeasedJob>> {
        self.jobs.claim().await
    }
    async fn renew(&self, job: &LeasedJob) -> mathesis_core::CoreResult<bool> {
        self.renewals.fetch_add(1, Ordering::SeqCst);
        if tokio::time::Instant::now() >= self.lose_at {
            Ok(false)
        } else {
            self.jobs.renew(job).await
        }
    }
    async fn complete(&self, _job: &LeasedJob) -> mathesis_core::CoreResult<()> {
        self.settlements.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn defer(&self, _job: &LeasedJob, _delay: Duration) -> mathesis_core::CoreResult<()> {
        self.settlements.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn retry(&self, _job: &LeasedJob, _error: &str) -> mathesis_core::CoreResult<()> {
        self.settlements.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test(start_paused = true)]
async fn lost_heartbeat_cancels_audit_and_never_settles_the_job() {
    let mut w = World::new(vec![completed("accept")], true).await;
    let job = prepare_audit(&w).await;
    w.worker.advisor = Some(Arc::new(FakeAdvisor {
        audit_delay: Duration::from_secs(41 * 60),
        ..Default::default()
    }));
    let jobs = Arc::new(LostHeartbeat {
        jobs: w.jobs.clone(),
        lose_at: tokio::time::Instant::now() + Duration::from_secs(60),
        renewals: AtomicUsize::new(0),
        settlements: AtomicUsize::new(0),
    });
    w.worker.jobs = jobs.clone();
    let started = tokio::time::Instant::now();
    w.worker.handle(job).await;
    assert_eq!(started.elapsed(), Duration::from_secs(5 * 60));
    let file = w.file().await;
    assert!(matches!(
        file.current().unwrap().audit.state,
        StepState::Running { .. }
    ));
    assert!(matches!(
        file.current().unwrap().advice.state,
        StepState::Pending
    ));
    assert_eq!(jobs.settlements.load(Ordering::SeqCst), 0);
    let renewals = jobs.renewals.load(Ordering::SeqCst);
    tokio::time::advance(Duration::from_secs(3600)).await;
    assert_eq!(
        jobs.renewals.load(Ordering::SeqCst),
        renewals,
        "heartbeat ended with the job"
    );
}

#[tokio::test]
async fn conjecture_pipeline_displays_before_letter_then_generates_and_regenerates_target() {
    use mathesis_core::model::{
        AuthenticationMethod, LeanStatementResponse, SubmissionKind, SubmissionStatus,
    };
    let answer = serde_json::json!({"recommendation":"accept","summary":"Open conjecture", "claims":[{"claim":"C2","conjecture":{
        "well_posed":true,"well_posed_reason":"Defined symbols", "status":"open","status_reason":"Open as far as available material establishes", "named_works":[],
        "escape":"content","escape_reason":"Needs a new bound","suggestions":["Give the boundary case"]}}]}).to_string();
    let w = World::new_kind(
        vec![Ok(OracleStatus::Completed {
            text: answer,
            model: None,
        })],
        true,
        SubmissionKind::Conjecture,
    )
    .await;
    let mut job = w.job().await;
    // Admission, report, audit; publication is applied in the next call before advice/letter.
    for _ in 0..4 {
        assert!(w.worker.referee_job(&job).await.unwrap());
        job = w.jobs.claim().await.unwrap().unwrap();
    }
    let current = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    let record = match current.status {
        SubmissionStatus::Accepted { record } => record,
        _ => panic!("not displayed"),
    };
    assert_eq!(
        w.worker
            .app
            .list_conjectures(None, None)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(w.file().await.letters.is_empty());
    assert!(w.worker.app.paper(&record).await.unwrap().claims.len() == 2);
    // Letter and target scheduling.
    assert!(w.worker.referee_job(&job).await.unwrap());
    job = w.jobs.claim().await.unwrap().unwrap();
    assert!(
        w.file().await.letters[0]
            .body
            .contains("Your conjecture is displayed")
    );
    assert!(w.worker.referee_job(&job).await.unwrap());
    // Claim whichever queued job is next, process both safely.
    for _ in 0..3 {
        let Some(next) = w.jobs.claim().await.unwrap() else {
            break;
        };
        w.worker.handle(next).await;
    }
    let current = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    assert_eq!(current.lean_statements.len(), 1);
    assert!(matches!(
        current.lean_statements[0].response,
        LeanStatementResponse::AwaitingAuthor
    ));
    assert!(
        w.worker
            .app
            .paper(&record)
            .await
            .unwrap()
            .lean_statements
            .is_empty()
    );
    let digest = &current.lean_statements[0].digest;
    w.worker
        .app
        .respond_lean_statement(
            &w.author,
            AuthenticationMethod::CookieSession,
            &w.paper.id,
            digest,
            false,
            "Keep the zero term explicit".into(),
        )
        .await
        .unwrap();
    let target_job = w.jobs.claim().await.unwrap().unwrap();
    assert_eq!(target_job.kind, JobKind::LeanStatement);
    w.worker.handle(target_job).await;
    let current = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    assert_eq!(current.lean_statements.len(), 2);
    assert_ne!(
        current.lean_statements[0].digest,
        current.lean_statements[1].digest
    );
    assert_eq!(
        w.formalizer.corrections.lock().await.as_slice(),
        [None, Some("Keep the zero term explicit".into())]
    );
}

#[tokio::test]
async fn accepted_paper_problem_runs_independent_referee_audit_and_author_target_confirmation() {
    use mathesis_core::{
        model::{AuthenticationMethod, SubmissionStatus},
        ports::ReviewQueue,
    };
    let problem = serde_json::json!({"recommendation":"accept","summary":"Open question in the paper", "claims":[{"claim":"C2","conjecture":{"well_posed":true,"well_posed_reason":"Defined symbols","status":"open","status_reason":"Not settled in the paper","escape":"content","escape_reason":"Needs a new bound"}}]}).to_string();
    let w = World::new(
        vec![
            completed("accept"),
            Ok(OracleStatus::Completed {
                text: problem,
                model: None,
            }),
        ],
        true,
    )
    .await;
    w.run_round().await;
    for kind in [
        JobKind::Referee,
        JobKind::OpenProblems,
        JobKind::LeanStatement,
    ] {
        if let Some(job) = w.jobs.current(&w.paper.id, kind).await {
            w.jobs.complete(&job).await.unwrap();
        }
    }
    // Keep this test focused on the independently leased candidate job.
    while let Some(job) = w.jobs.claim().await.unwrap() {
        w.jobs.complete(&job).await.unwrap();
    }
    let paper = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    let SubmissionStatus::Accepted { record } = paper.status else {
        panic!()
    };
    assert!(
        w.worker
            .app
            .list_conjectures(None, None)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    ReviewQueue::enqueue(&*w.stores, &w.paper.id, JobKind::OpenProblems)
        .await
        .unwrap();
    let mut job = w.jobs.claim().await.unwrap().unwrap();
    assert_eq!(job.kind, JobKind::OpenProblems);
    for _ in 0..2 {
        assert!(w.worker.open_problems_job(&job).await.unwrap());
        job = w.jobs.claim().await.unwrap().unwrap();
    }
    assert!(!w.worker.open_problems_job(&job).await.unwrap());
    w.jobs.complete(&job).await.unwrap();
    let files = w
        .worker
        .app
        .open_problem_files(&w.worker.reviewer, &w.paper.id)
        .await
        .unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].admitted && files[0].audit.is_some());
    let listed = w.worker.app.list_conjectures(None, None).await.unwrap();
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].claim.as_str(), "C2");
    assert!(listed.items[0].source.starts_with("from WP-"));
    // Staff-triggered candidates also work for old accepted papers with no
    // delivered primary letter; their own independent report supplies context.
    let mut primary = mathesis_core::ports::RefereeStore::get(&*w.stores, &w.paper.id)
        .await
        .unwrap()
        .unwrap();
    let expected = primary.revision;
    primary.rounds.clear();
    primary.revision += 1;
    mathesis_core::ports::RefereeStore::replace(&*w.stores, &primary, expected)
        .await
        .unwrap();
    let target_job = w.jobs.claim().await.unwrap().unwrap();
    assert_eq!(target_job.kind, JobKind::LeanStatement);
    w.worker.handle(target_job).await;
    let current = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    assert_eq!(current.lean_statements.len(), 1);
    assert!(w.worker.app.target(&record, &"C2".into()).await.is_err());
    w.worker
        .app
        .respond_lean_statement(
            &w.author,
            AuthenticationMethod::CookieSession,
            &w.paper.id,
            &current.lean_statements[0].digest,
            true,
            String::new(),
        )
        .await
        .unwrap();
    assert!(
        w.worker
            .app
            .target(&record, &"C2".into())
            .await
            .unwrap()
            .lean
            .contains("def wishpool_target_prop")
    );
    let before = w.oracle.submitted.load(Ordering::SeqCst);
    ReviewQueue::enqueue(&*w.stores, &w.paper.id, JobKind::OpenProblems)
        .await
        .unwrap();
    let job = w.jobs.claim().await.unwrap().unwrap();
    assert!(!w.worker.open_problems_job(&job).await.unwrap());
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), before);
}

struct ManagedTransport {
    created: AtomicUsize,
    calls: Mutex<Vec<(String, String)>>,
    answers: Mutex<std::collections::BTreeMap<String, String>>,
}
#[async_trait]
impl mathesis_review::cma::Transport for ManagedTransport {
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
        _key: Option<&str>,
    ) -> ReviewResult<mathesis_review::cma::TransportReply> {
        use serde_json::json;
        self.calls.lock().await.push((method.into(), path.into()));
        let mut response_id = None;
        let value = if path.ends_with("/agents") {
            let n = self.created.fetch_add(1, Ordering::SeqCst) + 1;
            json!({"id":format!("agt_{n}")})
        } else if method == "POST" && path.ends_with("/responses") {
            let body = body.unwrap();
            assert_eq!(body["stream"], true);
            let text = body["input"].as_str().unwrap();
            assert!(text.contains("Every gap is finite"));
            assert!(!text.contains("Network access is disabled"));
            let answer = if text.contains("Audit the GPT Pro referee report")
                || text.contains("Audit this conjecture referee report")
            {
                json!({"verdict":"accept","summary":"The compactness argument supplies new content.","claims":[{"claim":"C1","correctness":"correct","comment":"A compactness lemma supplies the proof.","shape":"content","witnesses":["A compactness lemma"],"referee_agreed":true},{"claim":"C2","correctness":"not_checked","comment":"An open question.","conjecture":{"well_posed":true,"well_posed_reason":"All quantities are defined","status":"open","status_reason":"No prior settlement found","escape":"content","escape_reason":"A new uniform bound is needed"}}]})
            } else if text.contains("Compare the original statement") {
                json!({"verdict":"faithful","reason":"The exact statement is preserved.","mismatch":""})
            } else if text.contains("You are advising") {
                json!({"summary":"Clarify the notation.","improvements":[],"formalization":[{"claim":"C1","plan":"A compactness proof","feasibility":"ready","effort":"small","mathlib":[],"lean_sketch":""}]})
            } else {
                json!({"subject":"Decision","body":"Dear A. Author,\n\nThe compactness argument is useful.","note":""})
            };
            let id = format!("resp_{}", self.created.load(Ordering::SeqCst));
            self.answers
                .lock()
                .await
                .insert(id.clone(), answer.to_string());
            response_id = Some(id);
            json!(null)
        } else if method == "GET" && path.starts_with("api/v2/responses/") {
            let id = path.rsplit('/').next().unwrap();
            let answer = self.answers.lock().await.get(id).unwrap().clone();
            json!({"status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":answer}]}]})
        } else if path.ends_with("/controls") {
            json!({"target_active":false,"target":{}})
        } else if method == "GET" && path.starts_with("api/v1/agents/") {
            if self
                .calls
                .lock()
                .await
                .iter()
                .any(|(method, p)| method == "DELETE" && p == path)
            {
                return Ok(mathesis_review::cma::TransportReply {
                    status: 404,
                    body: json!({"code":"not_found"}).to_string(),
                    response_id: None,
                });
            }
            json!({"status":"stopped"})
        } else {
            json!(null)
        };
        Ok(mathesis_review::cma::TransportReply {
            status: 200,
            body: if value.is_null() {
                String::new()
            } else {
                value.to_string()
            },
            response_id,
        })
    }
}
#[tokio::test]
async fn pipeline_uses_durable_cma_for_audit_advice_letter_and_hides_provider_state() {
    let mut w = World::new(vec![completed("accept")], true).await;
    let transport = Arc::new(ManagedTransport {
        created: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
        answers: Mutex::new(Default::default()),
    });
    let client = Arc::new(mathesis_review::cma::Client {
        transport: transport.clone(),
        workspace: "wks_test".into(),
        profile: None,
        poll: Duration::from_millis(1),
    });
    w.worker.cma_factory = Some(Arc::new(crate::cma_tokens::CmaFactory {
        operator: client.clone(),
        connection: None,
        app: w.worker.app.clone(),
    }));
    w.worker.advisor = Some(Arc::new(mathesis_review::cma::CmaAdvisor {
        client,
        timeout: Duration::from_secs(60),
        audit_timeout: Duration::from_secs(60),
    }));
    w.run_round().await;
    let file = w.file().await;
    assert_eq!(file.agent_steps.len(), 3);
    assert_eq!(
        file.current().unwrap().audit.engine.as_deref(),
        Some("nyxid-cma")
    );
    assert_eq!(file.letters.len(), 1);
    for step in file.agent_steps.values() {
        assert_eq!(step.cma_attempts.len(), 1);
        assert_eq!(
            step.cma_attempts[0].payer,
            mathesis_core::model::CmaPayer::Operator
        );
        assert_eq!(step.progress["stopped"], true);
        assert_eq!(step.progress["deleted"], true);
        assert!(
            step.progress["agent_id"]
                .as_str()
                .unwrap()
                .starts_with("agt_")
        );
        assert!(
            step.progress["response_id"]
                .as_str()
                .unwrap()
                .starts_with("resp_")
        );
    }
    let view = serde_json::to_string(&w.worker.app.referee(&w.author, &w.paper.id).await.unwrap())
        .unwrap();
    assert!(!view.contains("agent_steps") && !view.contains("agt_") && !view.contains("resp_"));
    assert_eq!(transport.created.load(Ordering::SeqCst), 3);
}
#[tokio::test]
async fn withdrawn_paper_cleans_up_its_saved_cma_turn_after_a_worker_restart() {
    use mathesis_review::cma::{RunState, RunStore};
    let mut w = World::new(vec![], true).await;
    let transport = Arc::new(ManagedTransport {
        created: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
        answers: Mutex::new(Default::default()),
    });
    w.worker.advisor = Some(Arc::new(mathesis_review::cma::CmaAdvisor {
        client: Arc::new(mathesis_review::cma::Client {
            transport: transport.clone(),
            workspace: "wks_test".into(),
            profile: None,
            poll: Duration::from_millis(1),
        }),
        timeout: Duration::from_secs(60),
        audit_timeout: Duration::from_secs(60),
    }));
    let job = w.job().await;
    let store = w.worker.agent_store(&job, &w.paper, "r1:audit:a0");
    store
        .save(
            serde_json::to_value(RunState {
                agent_id: Some("agt_previous_worker".into()),
                response_id: Some("resp_previous_worker".into()),
                deadline: u64::MAX,
                ..Default::default()
            })
            .unwrap(),
        )
        .await
        .unwrap();
    w.worker.app.withdraw(&w.author, &w.paper.id).await.unwrap();
    w.worker.cleanup_agents(&job).await.unwrap();
    let state = store.load().await.unwrap().unwrap();
    assert_eq!(state["deleted"], true);
    assert!(
        transport
            .calls
            .lock()
            .await
            .iter()
            .any(|(method, path)| method == "DELETE" && path.ends_with("agt_previous_worker"))
    );
    assert_eq!(transport.created.load(Ordering::SeqCst), 0);
}

struct TestCmaSetup;
#[async_trait]
impl mathesis_core::ports::CmaSetup for TestCmaSetup {
    fn enabled(&self) -> bool {
        true
    }
    async fn validate(
        &self,
        _: &mathesis_core::model::CmaBinding,
        _: &str,
    ) -> mathesis_core::CoreResult<mathesis_core::model::CmaWorkspaceCheck> {
        Err(mathesis_core::CoreError::Unavailable("not used".into()))
    }
}
struct TestCmaToken(String);
#[async_trait]
impl mathesis_review::cma::TokenSource for TestCmaToken {
    async fn token(&self) -> ReviewResult<String> {
        Ok(self.0.clone())
    }
}
type AccountRequest = (String, String, String, Option<String>, serde_json::Value);
type CreationReceipt = (u16, String, Option<String>);
struct AccountCma {
    author_token: String,
    operator_token: String,
    refuse: AtomicBool,
    setup_refusal: Mutex<Option<(u16, String)>>,
    request_event: tokio::sync::Notify,
    repeat_create_event: tokio::sync::Notify,
    create_gate: Mutex<Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>>,
    uncertain_create: AtomicBool,
    creation_outage: AtomicBool,
    create_receipts: Mutex<std::collections::BTreeMap<String, CreationReceipt>>,
    jwt_key: Vec<u8>,
    refresh_token: Mutex<String>,
    refresh_status: std::sync::atomic::AtomicU16,
    rotate: AtomicBool,
    refresh_gate: Mutex<Option<(Arc<tokio::sync::Barrier>, Arc<tokio::sync::Barrier>)>>,
    cleanup_failure: AtomicBool,
    requests: Mutex<Vec<AccountRequest>>,
    author: ManagedTransport,
    operator: ManagedTransport,
}
async fn account_cma_request(
    axum::extract::State(state): axum::extract::State<Arc<AccountCma>>,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
    method: axum::http::Method,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    use mathesis_review::cma::Transport;
    let token = headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap()
        .to_owned();
    let path = uri.path().trim_start_matches('/');
    let json = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    let key = headers
        .get("idempotency-key")
        .map(|h| h.to_str().unwrap().to_owned());
    state.requests.lock().await.push((
        token.clone(),
        method.to_string(),
        path.into(),
        key.clone(),
        json.clone(),
    ));
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.required_spec_claims.clear();
    let claims = jsonwebtoken::decode::<serde_json::Value>(
        &token,
        &jsonwebtoken::DecodingKey::from_secret(&state.jwt_key),
        &validation,
    )
    .ok();
    if claims.as_ref().is_none_or(|c| {
        c.claims["token_type"] != "access"
            || ["resource", "allowed_service_ids", "cnf", "act", "delegated"]
                .iter()
                .any(|k| c.claims.get(*k).is_some_and(|v| !v.is_null()))
    }) {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"code":"invalid_token", "detail": token})),
        )
            .into_response();
    }
    if path.ends_with("/agents") && method == "POST" {
        assert_eq!(
            json["agent_profile"],
            serde_json::json!({"id":"agp_published","revision":1})
        );
    }
    if method == "DELETE" && state.cleanup_failure.swap(false, Ordering::SeqCst) {
        return (
            axum::http::StatusCode::BAD_GATEWAY,
            axum::Json(serde_json::json!({"code":"provider_down", "detail": token})),
        )
            .into_response();
    }
    if token == state.author_token
        && method == "POST"
        && path.ends_with("/agents")
        && let Some((status, code)) = state.setup_refusal.lock().await.clone()
    {
        return (
            axum::http::StatusCode::from_u16(status).unwrap(),
            axum::Json(serde_json::json!({"code": code})),
        )
            .into_response();
    }
    if token == state.author_token
        && path.ends_with("/agents")
        && method == "POST"
        && state.refuse.swap(false, Ordering::SeqCst)
    {
        return (
            axum::http::StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({"code":"quota_exceeded","detail":token})),
        )
            .into_response();
    }
    let transport = if token == state.author_token {
        &state.author
    } else {
        assert_eq!(token, state.operator_token);
        &state.operator
    };
    if method == "POST"
        && path.ends_with("/agents")
        && let Some((status, body, _)) = state
            .create_receipts
            .lock()
            .await
            .get(key.as_ref().unwrap())
            .cloned()
    {
        if state.creation_outage.load(Ordering::SeqCst) {
            state.repeat_create_event.notify_one();
            return (
                axum::http::StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({"detail":token})),
            )
                .into_response();
        }
        return (axum::http::StatusCode::from_u16(status).unwrap(), body).into_response();
    }
    if method == "POST" && path.ends_with("/agents") {
        let gate = state.create_gate.lock().await.take();
        if let Some((entered, release)) = gate {
            entered.notify_one();
            release.notified().await;
        }
    }
    let reply = transport
        .request(
            method.as_str(),
            path,
            (!json.is_null()).then_some(json),
            key.as_deref(),
        )
        .await
        .unwrap();
    if method == "POST" && path.ends_with("/agents") {
        state.create_receipts.lock().await.insert(
            key.unwrap(),
            (reply.status, reply.body.clone(), reply.response_id.clone()),
        );
        if state.uncertain_create.swap(false, Ordering::SeqCst) {
            return (
                axum::http::StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({"detail": token})),
            )
                .into_response();
        }
    }
    let mut response = (
        axum::http::StatusCode::from_u16(reply.status).unwrap(),
        reply.body,
    )
        .into_response();
    if let Some(id) = reply.response_id {
        response
            .headers_mut()
            .insert("x-cma-response-id", id.parse().unwrap());
    }
    response
}
async fn author_cma_worker() -> (World, Arc<AccountCma>, Arc<mathesis_review::cma::Client>) {
    use base64::Engine as _;
    use mathesis_core::{
        model::{CmaBinding, CmaStatus},
        ports::CmaBindingStore,
    };
    let mut w = World::new_kind_with_cma(
        vec![],
        true,
        mathesis_core::model::SubmissionKind::Paper,
        true,
    )
    .await;
    let managed = || ManagedTransport {
        created: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
        answers: Mutex::new(Default::default()),
    };
    let jwt_key = rand::random::<[u8; 32]>().to_vec();
    let ordinary = |subject: &str| {
        jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
            &serde_json::json!({"sub":subject,"token_type":"access","exp":chrono::Utc::now().timestamp()+3600}),
            &jsonwebtoken::EncodingKey::from_secret(&jwt_key),
        )
        .unwrap()
    };
    let author_token = ordinary("author");
    let operator_token = ordinary("operator");
    let mock = Arc::new(AccountCma {
        refresh_token: Mutex::new(author_token.clone()),
        author_token,
        operator_token,
        jwt_key,
        refresh_status: std::sync::atomic::AtomicU16::new(200),
        rotate: AtomicBool::new(false),
        refresh_gate: Mutex::new(None),
        cleanup_failure: AtomicBool::new(false),
        uncertain_create: AtomicBool::new(false),
        creation_outage: AtomicBool::new(false),
        create_receipts: Mutex::new(Default::default()),
        refuse: AtomicBool::new(false),
        setup_refusal: Mutex::new(None),
        request_event: tokio::sync::Notify::new(),
        repeat_create_event: tokio::sync::Notify::new(),
        create_gate: Mutex::new(None),
        requests: Mutex::new(vec![]),
        author: managed(),
        operator: managed(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let discovery = serde_json::json!({"issuer":base,"authorization_endpoint":format!("{base}/authorize"),"token_endpoint":format!("{base}/token"),"jwks_uri":format!("{base}/keys")});
    let refresh_mock = mock.clone();
    let server = axum::Router::new()
        .route(
            "/.well-known/openid-configuration",
            axum::routing::get(move || async move { axum::Json(discovery) }),
        )
        .route(
            "/keys",
            axum::routing::get(|| async { axum::Json(serde_json::json!({"keys":[]})) }),
        )
        .route(
            "/token",
            axum::routing::post(move || {
                let state = refresh_mock.clone();
                async move {
                    let gate = state.refresh_gate.lock().await.clone();
                    if let Some((entered, release)) = gate { entered.wait().await; release.wait().await; }
                    let status = state.refresh_status.load(Ordering::SeqCst);
                    state.request_event.notify_one();
                    let body = if status == 200 {
                        let mut body = serde_json::json!({"access_token":state.refresh_token.lock().await.clone()});
                        if state.rotate.load(Ordering::SeqCst) { body["refresh_token"] = crate::auth::random_token().into(); }
                        body
                    } else { serde_json::json!({"error":if status==400 {"invalid_grant"} else {"temporarily_unavailable"}, "detail":state.author_token}) };
                    (axum::http::StatusCode::from_u16(status).unwrap(), axum::Json(body))
                }
            }),
        )
        .fallback(account_cma_request)
        .with_state(mock.clone());
    tokio::spawn(async move {
        axum::serve(listener, server).await.unwrap();
    });
    let nyxid = Arc::new(
        crate::auth::nyxid::NyxIdClient::discover(
            &url::Url::parse(&base).unwrap(),
            "test".into(),
            crate::auth::random_token(),
            format!("{base}/callback"),
        )
        .await
        .unwrap(),
    );
    let connection = Arc::new(crate::cma_connection::CmaConnection {
        tokens: Arc::new(crate::donations::MemoryDonorTokens::default()),
        cipher: crate::donations::TokenCipher::from_base64(
            &base64::engine::general_purpose::STANDARD.encode(rand::random::<[u8; 32]>()),
        )
        .unwrap(),
        nyxid,
        base_url: base.clone(),
        public_url: base.clone(),
    });
    let mut binding = CmaBinding::disconnected(w.author.person.clone(), chrono::Utc::now());
    binding.status = CmaStatus::Ready;
    binding.revision = 1;
    binding.generation = 1;
    binding.workspace_id = Some("wks_author".into());
    binding.reason = "author_connected".into();
    CmaBindingStore::replace(w.stores.as_ref(), &binding, 0)
        .await
        .unwrap();
    connection
        .tokens
        .put(
            &crate::cma_connection::slot(&w.author.person, 1),
            &connection
                .cipher
                .seal(&crate::auth::random_token())
                .unwrap(),
        )
        .await
        .unwrap();
    let operator = Arc::new(mathesis_review::cma::Client {
        transport: Arc::new(mathesis_review::cma::HttpTransport {
            base_url: base,
            tokens: Arc::new(TestCmaToken(mock.operator_token.clone())),
        }),
        workspace: "wks_operator".into(),
        profile: Some(serde_json::json!({"id":"agp_published","revision":1})),
        poll: Duration::from_millis(1),
    });
    w.worker.cma_factory = Some(Arc::new(crate::cma_tokens::CmaFactory {
        operator: operator.clone(),
        connection: Some(connection),
        app: w.worker.app.clone(),
    }));
    w.worker.advisor = Some(Arc::new(mathesis_review::cma::CmaAdvisor {
        client: operator.clone(),
        timeout: Duration::from_secs(30),
        audit_timeout: Duration::from_secs(30),
    }));
    (w, mock, operator)
}
#[tokio::test]
async fn every_cma_identity_uses_saved_author_account_workspace_and_hides_transport_tokens() {
    let (w, mock, client) = author_cma_worker().await;
    let job = w.job().await;
    let prompt = "Every gap is finite. You are advising";
    for label in [
        "r1:audit",
        "r1:advice",
        "r1:letter",
        "r1:formal",
        "target:C2:a0",
        "target:C2:a0:pass0",
        "target:C2:a0:pass0:faithfulness",
        "target:C2:a0:pass1",
        "target:C2:a0:pass1:faithfulness",
        "problem:C2:r1:audit",
    ] {
        let store = w.worker.agent_store(&job, &w.paper, label);
        client
            .answer(&store, prompt.into(), Duration::from_secs(30))
            .await
            .unwrap();
        // A restart retains both payer and original request identity, without creating again.
        client
            .answer(
                &w.worker.agent_store(&job, &w.paper, label),
                prompt.into(),
                Duration::from_secs(30),
            )
            .await
            .unwrap();
    }
    let file = w.file().await;
    assert_eq!(file.agent_steps.len(), 10);
    for step in file.agent_steps.values() {
        let attempt = &step.cma_attempts[0];
        assert_eq!(attempt.payer, mathesis_core::model::CmaPayer::Author);
        assert_eq!(attempt.workspace, "wks_author");
        assert_eq!(attempt.outcome, mathesis_core::model::CmaOutcome::Succeeded);
        assert_ne!(
            attempt.identity.kind,
            mathesis_core::model::CmaStepKind::LegacyUnknown
        );
    }
    assert_eq!(mock.author.created.load(Ordering::SeqCst), 10);
    assert_eq!(mock.operator.created.load(Ordering::SeqCst), 0);
    let requests = mock.requests.lock().await;
    for (token, _, path, _, body) in requests.iter() {
        assert_eq!(token, &mock.author_token);
        if path.ends_with("/agents") {
            assert_eq!(path, "api/v1/workspaces/wks_author/agents");
        }
        assert!(!body.to_string().contains(&mock.author_token));
        assert!(!body.to_string().contains(&mock.operator_token));
    }
    let stored = serde_json::to_string(&file).unwrap();
    let projected =
        serde_json::to_string(&w.worker.app.referee(&w.author, &w.paper.id).await.unwrap())
            .unwrap();
    for token in [&mock.author_token, &mock.operator_token] {
        assert!(!stored.contains(token));
        assert!(!projected.contains(token));
    }
}
#[tokio::test]
async fn author_quota_refusal_records_operator_successor_with_new_keys_and_no_relabel() {
    let (w, mock, client) = author_cma_worker().await;
    mock.refuse.store(true, Ordering::SeqCst);
    let job = w.job().await;
    client
        .answer(
            &w.worker.agent_store(&job, &w.paper, "r1:audit"),
            "Every gap is finite. You are advising".into(),
            Duration::from_secs(30),
        )
        .await
        .unwrap();
    let file = w.file().await;
    let step = file.agent_steps.values().next().unwrap();
    assert_eq!(step.cma_attempts.len(), 2);
    assert_eq!(
        step.cma_attempts[0].payer,
        mathesis_core::model::CmaPayer::Author
    );
    assert_eq!(
        step.cma_attempts[0].failure,
        Some(mathesis_core::model::CmaFailure::Quota)
    );
    assert_eq!(
        step.cma_attempts[1].payer,
        mathesis_core::model::CmaPayer::Operator
    );
    assert_eq!(step.cma_attempts[1].fallback_from, Some(0));
    assert_eq!(step.cma_attempts[1].reason, "quota");
    let requests = mock.requests.lock().await;
    let creates: Vec<_> = requests
        .iter()
        .filter(|(_, method, path, _, _)| method == "POST" && path.ends_with("/agents"))
        .collect();
    assert_eq!(creates[0].0, mock.author_token);
    assert_eq!(creates.last().unwrap().0, mock.operator_token);
    assert_ne!(creates[0].3, creates.last().unwrap().3);
    assert_eq!(
        creates.last().unwrap().2,
        "api/v1/workspaces/wks_operator/agents"
    );
}
#[tokio::test]
async fn historical_author_cleanup_uses_original_account_after_operator_successor() {
    use mathesis_review::cma::{FailureKind, RunState, RunStore};
    let (w, mock, client) = author_cma_worker().await;
    let job = w.job().await;
    let store = w.worker.agent_store(&job, &w.paper, "r1:audit");
    store.execution().await.unwrap();
    store
        .save(
            serde_json::to_value(RunState {
                agent_id: Some("agt_original".into()),
                failure: Some("CMA account refusal".into()),
                typed_failure: Some(FailureKind::Quota),
                deadline: u64::MAX,
                ..Default::default()
            })
            .unwrap(),
        )
        .await
        .unwrap();
    assert!(store.fallback(FailureKind::Quota).await.unwrap());
    client
        .answer(
            &store,
            "Every gap is finite. You are advising".into(),
            Duration::from_secs(30),
        )
        .await
        .unwrap();
    w.worker.cleanup_agents(&job).await.unwrap();
    let requests = mock.requests.lock().await;
    for (token, _, path, _, _) in requests
        .iter()
        .filter(|(_, _, path, _, _)| path.contains("agt_original"))
    {
        assert_eq!(token, &mock.author_token, "{path}");
    }
    assert!(
        requests
            .iter()
            .any(|(token, method, path, _, _)| token == &mock.author_token
                && method == "DELETE"
                && path.ends_with("agt_original"))
    );
    let file = w.file().await;
    assert_eq!(
        file.agent_steps.values().next().unwrap().cma_attempts[0].progress["deleted"],
        true
    );
}

#[tokio::test]
async fn refresh_outages_keep_payer_key_body_and_binding() {
    for status in [429, 500, 503, 403, 401, 204] {
        let (w, mock, client) = author_cma_worker().await;
        mock.refresh_status.store(status, Ordering::SeqCst);
        let job = w.job().await;
        let before = w.worker.app.cma_binding(&w.author.person).await.unwrap();
        let store = w.worker.agent_store(&job, &w.paper, "r1:audit");
        tokio::select! {
            result = client.answer(&store, "Every gap is finite. You are advising".into(), Duration::from_secs(30)) => panic!("unexpected completion: {result:?}"),
            () = async { mock.request_event.notified().await; mock.request_event.notified().await; } => {}
        }
        let initial = w.file().await;
        let intent = initial.agent_steps.values().next().unwrap().progress["create_intent"].clone();
        assert_eq!(
            initial
                .agent_steps
                .values()
                .next()
                .unwrap()
                .cma_attempts
                .len(),
            1
        );
        assert_eq!(
            w.worker.app.cma_binding(&w.author.person).await.unwrap(),
            before
        );
        mock.refresh_status.store(200, Ordering::SeqCst);
        client
            .answer(
                &w.worker.agent_store(&job, &w.paper, "r1:audit"),
                "Every gap is finite. You are advising".into(),
                Duration::from_secs(30),
            )
            .await
            .unwrap();
        let file = w.file().await;
        let step = file.agent_steps.values().next().unwrap();
        assert_eq!(step.cma_attempts.len(), 1);
        assert_eq!(
            step.cma_attempts[0].payer,
            mathesis_core::model::CmaPayer::Author
        );
        assert_eq!(step.progress["create_intent"], intent);
        assert_eq!(mock.operator.created.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn disconnect_during_refresh_without_rotation_fences_returned_token() {
    let (w, mock, _) = author_cma_worker().await;
    let entered = Arc::new(tokio::sync::Barrier::new(2));
    let release = Arc::new(tokio::sync::Barrier::new(2));
    *mock.refresh_gate.lock().await = Some((entered.clone(), release.clone()));
    let job = w.job().await;
    let attempt = w
        .worker
        .app
        .admit_cma_attempt(
            &w.worker.auditor_account,
            &job,
            w.worker.jobs.as_ref(),
            "race",
            "r1:audit",
            "wks_operator",
        )
        .await
        .unwrap();
    let client = w
        .worker
        .cma_factory
        .as_ref()
        .unwrap()
        .client(&attempt)
        .unwrap();
    let request = client
        .transport
        .request("GET", "api/v1/workspaces/wks_author", None, None);
    let disconnect = async {
        entered.wait().await;
        w.worker.app.disconnect_cma(&w.author).await.unwrap();
        release.wait().await;
    };
    let (result, ()) = tokio::join!(request, disconnect);
    assert!(matches!(
        result,
        Err(ReviewError::CmaRefusal(
            mathesis_review::cma::FailureKind::Auth
        ))
    ));
    assert!(mock.requests.lock().await.is_empty());
}

#[tokio::test]
async fn simultaneous_validation_and_author_refresh_rotation_is_transient() {
    let (w, mock, _) = author_cma_worker().await;
    mock.rotate.store(true, Ordering::SeqCst);
    let entered = Arc::new(tokio::sync::Barrier::new(3));
    let release = Arc::new(tokio::sync::Barrier::new(3));
    *mock.refresh_gate.lock().await = Some((entered.clone(), release.clone()));
    let connection = w
        .worker
        .cma_factory
        .as_ref()
        .unwrap()
        .connection
        .as_ref()
        .unwrap();
    let job = w.job().await;
    let attempt = w
        .worker
        .app
        .admit_cma_attempt(
            &w.worker.auditor_account,
            &job,
            w.worker.jobs.as_ref(),
            "race",
            "r1:audit",
            "wks_operator",
        )
        .await
        .unwrap();
    let client = w
        .worker
        .cma_factory
        .as_ref()
        .unwrap()
        .client(&attempt)
        .unwrap();
    let validation = connection.access_token(&w.author.person, 1);
    let execution = client
        .transport
        .request("GET", "api/v1/workspaces/wks_author", None, None);
    let schedule = async {
        entered.wait().await;
        release.wait().await;
    };
    let (left, right, ()) = tokio::join!(validation, execution, schedule);
    assert_ne!(left.is_ok(), right.is_ok());
    if let Err(error) = left {
        assert!(matches!(error, mathesis_core::CoreError::Unavailable(_)));
    }
    if let Err(error) = right {
        assert!(matches!(error, ReviewError::Transport(_)));
    }
    assert_eq!(
        w.worker
            .app
            .cma_binding(&w.author.person)
            .await
            .unwrap()
            .status,
        mathesis_core::model::CmaStatus::Ready
    );
    assert_eq!(w.file().await.agent_steps["race"].cma_attempts.len(), 1);
}

#[tokio::test]
async fn restricted_jwts_and_opaque_tokens_are_refused_with_explicit_successor() {
    for restriction in [
        "resource",
        "allowed_service_ids",
        "cnf",
        "act",
        "delegated",
        "opaque",
        "missing_token_type",
        "wrong_token_type",
    ] {
        let (w, mock, client) = author_cma_worker().await;
        let mut claims = jsonwebtoken::decode::<serde_json::Value>(
            &mock.author_token,
            &jsonwebtoken::DecodingKey::from_secret(&mock.jwt_key),
            &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
        )
        .unwrap()
        .claims;
        match restriction {
            "missing_token_type" => {
                claims.as_object_mut().unwrap().remove("token_type");
            }
            "wrong_token_type" => {
                claims["token_type"] = "refresh".into();
            }
            _ => {
                claims[restriction] = serde_json::json!(true);
            }
        }
        *mock.refresh_token.lock().await = if restriction == "opaque" {
            crate::auth::random_token()
        } else {
            jsonwebtoken::encode(
                &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
                &claims,
                &jsonwebtoken::EncodingKey::from_secret(&mock.jwt_key),
            )
            .unwrap()
        };
        let job = w.job().await;
        client
            .answer(
                &w.worker.agent_store(&job, &w.paper, "r1:audit"),
                "Every gap is finite. You are advising".into(),
                Duration::from_secs(30),
            )
            .await
            .unwrap();
        let file = w.file().await;
        let attempts = &file.agent_steps.values().next().unwrap().cma_attempts;
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0].payer, mathesis_core::model::CmaPayer::Author);
        assert_eq!(
            attempts[0].failure,
            Some(mathesis_core::model::CmaFailure::Auth)
        );
        assert_eq!(attempts[1].payer, mathesis_core::model::CmaPayer::Operator);
        assert_eq!(mock.author.created.load(Ordering::SeqCst), 0);
        assert_eq!(mock.operator.created.load(Ordering::SeqCst), 1);
    }
}

struct HttpCmaFormalizer {
    client: Arc<mathesis_review::cma::Client>,
    checker: FakeFormalizer,
}
#[async_trait]
impl Formalizer for HttpCmaFormalizer {
    fn engine(&self) -> &str {
        "nyxid-cma+test-checker"
    }
    fn model(&self) -> &str {
        "cma-profile"
    }
    async fn formalize(&self, _: &FormalInput, _: &std::path::Path) -> ReviewResult<FormalOut> {
        unreachable!("worker must use durable store")
    }
    async fn formalize_with_store(
        &self,
        input: &FormalInput,
        work: &std::path::Path,
        store: &dyn mathesis_review::cma::RunStore,
    ) -> ReviewResult<FormalOut> {
        // Replace only the credential-free local checker; drive actual worker dispatch and CMA transport.
        self.client
            .answer(
                store,
                mathesis_review::referee_prompts::formalize(input, "test pinned toolchain"),
                Duration::from_secs(30),
            )
            .await?;
        self.checker.formalize(input, work).await
    }
}

#[tokio::test]
async fn real_worker_dispatches_all_submission_cma_steps_for_author_and_operator_and_restarts_uncertain_creation()
 {
    use mathesis_core::model::CmaStepKind;
    let problem = serde_json::json!({"recommendation":"accept","summary":"Open question", "claims":[{"claim":"C2","conjecture":{"well_posed":true,"well_posed_reason":"Defined symbols","status":"open","status_reason":"Not settled","escape":"content","escape_reason":"Needs a new bound"}}]}).to_string();
    for author_pays in [true, false] {
        let (mut w, mock, client) = author_cma_worker().await;
        if !author_pays {
            w.worker.app.disconnect_cma(&w.author).await.unwrap();
        }
        w.oracle.outcomes.lock().await.extend([
            completed("accept"),
            Ok(OracleStatus::Completed {
                text: problem.clone(),
                model: None,
            }),
        ]);
        w.worker.formalizer = Some(Arc::new(HttpCmaFormalizer {
            client: client.clone(),
            checker: FakeFormalizer::default(),
        }));
        // The provider admitted the author mutation, but its first receipt is lost.
        let mut job = w.job().await;
        for _ in 0..2 {
            assert!(w.worker.referee_job(&job).await.unwrap());
            job = w.claim_referee().await;
        }
        mock.uncertain_create.store(true, Ordering::SeqCst);
        mock.creation_outage.store(true, Ordering::SeqCst);
        tokio::select! {
            result = w.worker.referee_job(&job) => panic!("unexpected completion: {result:?}"),
            () = mock.repeat_create_event.notified() => {}
        }
        assert_eq!(
            w.file()
                .await
                .agent_steps
                .values()
                .next()
                .unwrap()
                .cma_attempts
                .len(),
            1
        );
        mock.creation_outage.store(false, Ordering::SeqCst);
        for _ in 0..18 {
            if !w.worker.referee_job(&job).await.unwrap() {
                break;
            }
            job = w.claim_referee().await;
        }
        let file = w.file().await;
        assert_eq!(file.letters.len(), 1, "{:?}", file.current());
        assert!(
            file.agent_steps
                .values()
                .any(|s| s.cma_attempts[0].identity.kind == CmaStepKind::LeanProbe)
        );
        // Worker restart: a new AgentStore is constructed on every dispatch above.
        // Complete parked referee job so the independent problem job is next.
        if let Some(job) = w.jobs.current(&w.paper.id, JobKind::Referee).await {
            w.jobs.complete(&job).await.unwrap();
        }
        // The focused referee claimant parks unrelated jobs; re-admit the candidate job now.
        mathesis_core::ports::ReviewQueue::enqueue(
            w.stores.as_ref(),
            &w.paper.id,
            JobKind::OpenProblems,
        )
        .await
        .unwrap();
        let mut seen_target = false;
        for _ in 0..12 {
            let Some(job) = w.jobs.claim().await.unwrap() else {
                break;
            };
            match job.kind {
                JobKind::OpenProblems => {
                    if !w.worker.open_problems_job(&job).await.unwrap() {
                        w.jobs.complete(&job).await.unwrap();
                    }
                }
                JobKind::LeanStatement => {
                    assert!(!w.worker.lean_statement_job(&job).await.unwrap());
                    w.jobs.complete(&job).await.unwrap();
                    seen_target = true;
                }
                _ => {
                    w.jobs.complete(&job).await.unwrap();
                }
            }
        }
        assert!(seen_target);
        let file = w.file().await;
        for kind in [
            CmaStepKind::Audit,
            CmaStepKind::Advice,
            CmaStepKind::Letter,
            CmaStepKind::LeanProbe,
            CmaStepKind::ConjectureTarget,
            CmaStepKind::ProblemAudit,
            CmaStepKind::TargetFaithfulness,
        ] {
            let (key, step) = file
                .agent_steps
                .iter()
                .find(|(_, s)| s.cma_attempts[0].identity.kind == kind)
                .unwrap_or_else(|| panic!("missing {kind:?}"));
            assert_eq!(step.cma_attempts.len(), 1);
            assert_eq!(
                step.cma_attempts[0].payer,
                if author_pays {
                    mathesis_core::model::CmaPayer::Author
                } else {
                    mathesis_core::model::CmaPayer::Operator
                }
            );
            assert_eq!(
                step.cma_attempts[0].outcome,
                mathesis_core::model::CmaOutcome::Succeeded
            );
            let requests = mock.requests.lock().await;
            let create = requests
                .iter()
                .find(|r| r.3.as_deref() == Some(&format!("{key}:create")))
                .unwrap();
            assert_eq!(
                create.0,
                if author_pays {
                    mock.author_token.clone()
                } else {
                    mock.operator_token.clone()
                }
            );
            assert_eq!(
                create.2,
                format!(
                    "api/v1/workspaces/{}/agents",
                    step.cma_attempts[0].workspace
                )
            );
            assert_eq!(create.4, step.progress["create_intent"][1]);
        }
        let requests = mock.requests.lock().await;
        let creates: Vec<_> = requests
            .iter()
            .filter(|r| r.1 == "POST" && r.2.ends_with("/agents"))
            .collect();
        let first_key = creates[0].3.clone();
        let repeated: Vec<_> = creates.iter().filter(|r| r.3 == first_key).collect();
        assert!(repeated.len() >= 2);
        assert!(
            repeated
                .iter()
                .all(|r| r.4 == creates[0].4 && r.0 == creates[0].0)
        );
        assert_eq!(
            if author_pays {
                mock.author.created.load(Ordering::SeqCst)
            } else {
                mock.operator.created.load(Ordering::SeqCst)
            },
            7
        );
        assert_eq!(
            if author_pays {
                mock.operator.created.load(Ordering::SeqCst)
            } else {
                mock.author.created.load(Ordering::SeqCst)
            },
            0
        );
        drop(requests);
        w.worker
            .app
            .respond_to_formalization(&w.author, &w.paper.id, &"C1".into(), true, String::new())
            .await
            .unwrap();
        let target_job = loop {
            let job = w.jobs.claim().await.unwrap().unwrap();
            if job.kind == JobKind::LeanStatement {
                break job;
            }
            w.jobs.complete(&job).await.unwrap();
        };
        assert!(!w.worker.lean_statement_job(&target_job).await.unwrap());
        let file = w.file().await;
        for kind in [
            CmaStepKind::ConjectureTarget,
            CmaStepKind::TargetFaithfulness,
        ] {
            let step = file
                .agent_steps
                .values()
                .find(|s| {
                    s.cma_attempts[0].identity.kind == kind
                        && s.cma_attempts[0]
                            .identity
                            .claim
                            .as_ref()
                            .is_some_and(|c| c.as_str() == "C1")
                })
                .expect("proved paper target and audit must use CMA");
            assert_eq!(
                step.cma_attempts[0].payer,
                if author_pays {
                    mathesis_core::model::CmaPayer::Author
                } else {
                    mathesis_core::model::CmaPayer::Operator
                }
            );
        }
    }
}

#[derive(Clone, Default)]
struct CapturedTrace(Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for CapturedTrace {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedTrace {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}
#[tokio::test]
async fn provider_token_sentinels_never_enter_tracing_prompts_state_or_api() {
    use tracing::instrument::WithSubscriber;
    let capture = CapturedTrace::default();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(capture.clone())
        .finish();
    let (w, mock, client) = author_cma_worker().await;
    async {
        let job = w.job().await;
        mock.refresh_status.store(503, Ordering::SeqCst);
        let store = w.worker.agent_store(&job, &w.paper, "r1:audit");
        tokio::select! {
            result = client.answer(&store, "Every gap is finite. You are advising".into(), Duration::from_secs(30)) => panic!("unexpected completion: {result:?}"),
            () = async { mock.request_event.notified().await; mock.request_event.notified().await; } => {}
        }
        tracing::warn!("refresh retry");
        mock.refresh_status.store(200, Ordering::SeqCst);
        mock.refuse.store(true, Ordering::SeqCst);
        mock.cleanup_failure.store(true, Ordering::SeqCst);
        client
            .answer(
                &w.worker.agent_store(&job, &w.paper, "r1:audit"),
                "Every gap is finite. You are advising".into(),
                Duration::from_secs(30),
            )
            .await
            .unwrap();
        w.worker.cleanup_agents(&job).await.unwrap();
    }
    .with_subscriber(subscriber)
    .await;
    let logs = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("refresh retry"));
    assert!(logs.contains("cleanup"));
    let state = serde_json::to_string(&w.file().await).unwrap();
    let api = serde_json::to_string(&w.worker.app.referee(&w.author, &w.paper.id).await.unwrap())
        .unwrap();
    let requests = mock.requests.lock().await;
    for token in [&mock.author_token, &mock.operator_token] {
        for text in [&logs, &state, &api] {
            assert!(!text.contains(token));
        }
        for request in requests.iter() {
            assert!(!request.4.to_string().contains(token));
        }
    }
}

#[tokio::test]
async fn definitive_invalid_grant_refresh_records_auth_fallback() {
    let (w, mock, client) = author_cma_worker().await;
    mock.refresh_status.store(400, Ordering::SeqCst);
    let job = w.job().await;
    client
        .answer(
            &w.worker.agent_store(&job, &w.paper, "r1:audit"),
            "Every gap is finite. You are advising".into(),
            Duration::from_secs(30),
        )
        .await
        .unwrap();
    let file = w.file().await;
    let attempts = &file.agent_steps.values().next().unwrap().cma_attempts;
    assert_eq!(attempts.len(), 2);
    assert_eq!(
        attempts[0].failure,
        Some(mathesis_core::model::CmaFailure::Auth)
    );
    assert_eq!(attempts[1].payer, mathesis_core::model::CmaPayer::Operator);
    assert_eq!(mock.author.created.load(Ordering::SeqCst), 0);
    assert_eq!(mock.operator.created.load(Ordering::SeqCst), 1);
}

struct InspectionFake {
    outputs: Mutex<VecDeque<mathesis_review::advisor::InspectionOut>>,
    calls: AtomicUsize,
}
#[async_trait]
impl Advisor for InspectionFake {
    fn engine(&self) -> &str {
        "fake-cma"
    }
    fn model(&self) -> &str {
        "fake"
    }
    async fn advise(&self, _: &AdvisorInput) -> ReviewResult<AdviceOut> {
        unreachable!()
    }
    async fn draft_letter(
        &self,
        _: &AdvisorInput,
        _: Option<&AdviceOut>,
    ) -> ReviewResult<LetterOut> {
        unreachable!()
    }
    async fn inspect_with_store(
        &self,
        input: &AdvisorInput,
        question: &str,
        _: &dyn mathesis_review::cma::RunStore,
    ) -> ReviewResult<mathesis_review::advisor::InspectionOut> {
        assert!(input.text.as_ref().is_some_and(|t| t.contains("Every gap")));
        assert!(!question.is_empty());
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .outputs
            .lock()
            .await
            .pop_front()
            .expect("one inspection per stage"))
    }
}
fn inspection(verdict: &str) -> mathesis_review::advisor::InspectionOut {
    mathesis_review::advisor::InspectionOut {
        verdict: verdict.into(),
        reason: "The statement needs the zero case.".into(),
        mismatch: if verdict == "faithful" {
            String::new()
        } else {
            "The source includes zero.".into()
        },
        evidence: "At n=0, left=1 and right=0.".into(),
        basis: "offline".into(),
    }
}
#[tokio::test]
async fn statement_targets_run_faithfulness_audit_and_regenerate_exactly_once_before_confirmation()
{
    use mathesis_core::model::{FaithfulnessVerdict, SubmissionStatus};
    let mut w = World::new(vec![completed("accept")], true).await;
    w.run_round().await;
    let paper = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    assert!(matches!(paper.status, SubmissionStatus::Accepted { .. }));
    w.worker
        .app
        .respond_to_formalization(&w.author, &paper.id, &"C1".into(), true, String::new())
        .await
        .unwrap();
    let inspector = Arc::new(InspectionFake {
        outputs: Mutex::new(VecDeque::from([
            inspection("weaker"),
            inspection("different"),
        ])),
        calls: AtomicUsize::new(0),
    });
    w.worker.advisor = Some(inspector.clone());
    // Drain other jobs without using the inspection-only fake for review.
    let job = loop {
        let job = w.jobs.claim().await.unwrap().unwrap();
        if job.kind == JobKind::LeanStatement {
            break job;
        }
        w.jobs.complete(&job).await.unwrap();
    };
    assert!(!w.worker.lean_statement_job(&job).await.unwrap());
    let paper = w.worker.app.submission(&w.author, &paper.id).await.unwrap();
    assert_eq!(inspector.calls.load(Ordering::SeqCst), 2);
    assert_eq!(paper.lean_statements.len(), 1);
    let audit = paper.lean_statements[0].faithfulness.as_ref().unwrap();
    assert_eq!(audit.verdict, FaithfulnessVerdict::Different);
    assert!(audit.regenerated);
    assert!(matches!(
        paper.lean_statements[0].response,
        mathesis_core::model::LeanStatementResponse::AwaitingAuthor
    ));
    let corrections = w.formalizer.corrections.lock().await;
    assert!(
        corrections
            .last()
            .unwrap()
            .as_ref()
            .unwrap()
            .contains("source includes zero")
    );
    // Replay has no new generation or audit.
    assert!(!w.worker.lean_statement_job(&job).await.unwrap());
    assert_eq!(inspector.calls.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn text_reports_use_fake_audit_and_confirmed_review_starts_one_new_round() {
    use mathesis_core::model::{ErrorSubject, ErrorVerdict, NewErrorReport, SubmissionStatus};
    let mut w = World::new(vec![completed("accept")], true).await;
    w.run_round().await;
    let paper = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    let SubmissionStatus::Accepted { record } = paper.status else {
        panic!()
    };
    let inspector = Arc::new(InspectionFake {
        outputs: Mutex::new(VecDeque::from([
            inspection("confirmed"),
            inspection("refuted"),
            inspection("unclear"),
        ])),
        calls: AtomicUsize::new(0),
    });
    w.worker.advisor = Some(inspector.clone());
    for expected in [
        ErrorVerdict::Confirmed,
        ErrorVerdict::Refuted,
        ErrorVerdict::Unclear,
    ] {
        let report = w
            .worker
            .app
            .file_error_report(
                &w.author,
                &record,
                NewErrorReport {
                    subject: ErrorSubject::Review,
                    text: "Check the zero case".into(),
                    solution: None,
                    as_agent: None,
                },
            )
            .await
            .unwrap();
        let job = loop {
            let job = w.jobs.claim().await.unwrap().unwrap();
            if job.kind == JobKind::ErrorReports {
                break job;
            }
            w.jobs.complete(&job).await.unwrap();
        };
        w.worker.error_reports_job(&job).await.unwrap();
        w.jobs.complete(&job).await.unwrap();
        let result = w
            .worker
            .app
            .error_report(Some(&w.author), &report.id)
            .await
            .unwrap();
        assert_eq!(result.resolution.unwrap().verdict, expected);
        assert_eq!(
            w.worker.app.error_report(None, &report.id).await.is_ok(),
            expected == ErrorVerdict::Confirmed
        );
    }
    let restarted = w
        .worker
        .app
        .begin_referee_round(&w.worker.referee_account, &w.paper.id)
        .await
        .unwrap();
    assert_eq!(restarted.rounds.len(), 2);
    assert_eq!(
        w.worker
            .app
            .begin_referee_round(&w.worker.referee_account, &w.paper.id)
            .await
            .unwrap()
            .rounds
            .len(),
        2
    );
    let evidence = w
        .worker
        .app
        .recheck_evidence(&w.worker.auditor_account, &w.paper.id)
        .await
        .unwrap();
    assert!(evidence.contains("Check the zero case"));
    assert!(
        !serde_json::to_string(&w.worker.app.paper(&record).await.unwrap())
            .unwrap()
            .contains("one inspection")
    );
}

#[tokio::test]
async fn setup_refusals_append_exactly_one_operator_successor() {
    use mathesis_review::cma::{FailureKind, RunStore};
    for (status, code) in [
        (404, "workspace_not_found"),
        (409, "connections_required"),
        (409, "trust_confirmation_required"),
    ] {
        let (w, mock, client) = author_cma_worker().await;
        *mock.setup_refusal.lock().await = Some((status, code.into()));
        let job = w.job().await;
        let store = w.worker.agent_store(&job, &w.paper, "r1:audit");
        for _ in 0..2 {
            client
                .answer(
                    &store,
                    "Every gap is finite. You are advising".into(),
                    Duration::from_secs(30),
                )
                .await
                .unwrap();
        }
        assert!(!store.fallback(FailureKind::Setup).await.unwrap());
        let file = w.file().await;
        let attempts = &file.agent_steps.values().next().unwrap().cma_attempts;
        assert_eq!(attempts.len(), 2, "{code}");
        assert_eq!(
            attempts[0].failure,
            Some(mathesis_core::model::CmaFailure::Setup)
        );
        assert_eq!(attempts[1].payer, mathesis_core::model::CmaPayer::Operator);
        assert_eq!(attempts[1].fallback_from, Some(0));
        assert_eq!(mock.author.created.load(Ordering::SeqCst), 0);
        assert_eq!(mock.operator.created.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn mid_run_aggregate_change_propagates_typed_fence_without_retry_or_fallback() {
    use mathesis_core::ports::RefereeStore;
    let (w, mock, client) = author_cma_worker().await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    *mock.create_gate.lock().await = Some((entered.clone(), release.clone()));
    let job = w.job().await;
    let store = w.worker.agent_store(&job, &w.paper, "r1:audit");
    let (result, ()) = tokio::join!(
        async {
            tokio::time::timeout(
                Duration::from_secs(5),
                client.answer(
                    &store,
                    "Every gap is finite. You are advising".into(),
                    Duration::from_secs(30),
                ),
            )
            .await
            .expect("fence must return promptly")
        },
        async {
            entered.notified().await;
            let mut file = w.file().await;
            let revision = file.revision;
            file.recheck_requests.push("concurrent-update".into());
            RefereeStore::replace(w.stores.as_ref(), &file, revision)
                .await
                .unwrap();
            release.notify_one();
        }
    );
    assert!(matches!(result, Err(ReviewError::PersistenceFence)));
    let file = w.file().await;
    let step = file.agent_steps.values().next().unwrap();
    assert_eq!(step.cma_attempts.len(), 1);
    assert!(step.progress["failure"].is_null());
    assert_eq!(mock.author.created.load(Ordering::SeqCst), 1);
    assert_eq!(mock.operator.created.load(Ordering::SeqCst), 0);
    assert_eq!(mock.requests.lock().await.len(), 1);
}
#[tokio::test]
async fn venue_attempts_and_error_audits_never_use_ready_author_binding() {
    use mathesis_core::model::{CmaPayer, CmaStepKind};
    let (w, mock, client) = author_cma_worker().await;
    let job = w.job().await;
    for (label, kind) in [
        ("agent-attempt:C2:digest", CmaStepKind::VenueAttempt),
        ("error-report:report1", CmaStepKind::ErrorReportAudit),
    ] {
        let store = w.worker.agent_store(&job, &w.paper, label);
        client
            .answer(
                &store,
                "Every gap is finite. You are advising".into(),
                Duration::from_secs(30),
            )
            .await
            .unwrap();
        let file = w.file().await;
        let attempt = &file
            .agent_steps
            .values()
            .find(|s| s.cma_attempts[0].identity.kind == kind)
            .unwrap()
            .cma_attempts[0];
        assert_eq!(attempt.payer, CmaPayer::Operator);
        assert_eq!(attempt.reason, "operator_step");
        assert_eq!(attempt.workspace, "wks_operator");
        assert!(attempt.owner.is_none());
    }
    assert_eq!(mock.author.created.load(Ordering::SeqCst), 0);
    assert_eq!(mock.operator.created.load(Ordering::SeqCst), 2);
}

async fn replay_pre_byok_terminal_state(
    referees: Option<Arc<dyn mathesis_core::ports::RefereeStore>>,
) {
    use mathesis_core::{
        model::{AgentStep, RefereeFile},
        ports::RefereeStore,
    };
    use mathesis_review::cma::{RunState, RunStore};
    use sha2::{Digest, Sha256};
    let mut w = World::new(vec![], true).await;
    let referees: Arc<dyn RefereeStore> = referees.unwrap_or_else(|| w.stores.clone());
    let mut ports = w.stores.ports(Arc::new(SystemClock), Arc::new(LatexReader));
    ports.referees = referees.clone();
    w.worker.app = App::new(ports, Policy::default(), BTreeSet::new());
    let job = w.job().await;
    let store = w.worker.agent_store(&job, &w.paper, "r1:audit");
    let transport = Arc::new(ManagedTransport {
        created: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
        answers: Mutex::new(Default::default()),
    });
    let client = mathesis_review::cma::Client {
        transport: transport.clone(),
        workspace: "unused".into(),
        profile: None,
        poll: Duration::from_millis(1),
    };
    // Literal pre-change JSON: no typed_failure or payer history. Load/save must
    // preserve the settled answer even though RunState now serializes defaults.
    let mut progress: serde_json::Value = serde_json::from_str(r#"{"agent_id":"agt_legacy","response_id":null,"next_chunk":1,"deadline":1,"input":"","answer":"{\"legacy\":true}","failure":null,"stopped":true,"deleted":true,"cancel_intent":null,"create_intent":null}"#).unwrap();
    progress["input"] = format!("{:x}", Sha256::digest(b"legacy prompt")).into();
    let mut file = RefereeFile::new(w.paper.id.clone());
    file.agent_steps.insert(
        store.key().into(),
        AgentStep {
            version: 1,
            claims_revision: w.paper.claims_revision,
            cma_attempts: vec![],
            progress,
        },
    );
    referees.insert(&file).await.unwrap();
    assert!(
        referees
            .get(&w.paper.id)
            .await
            .unwrap()
            .unwrap()
            .agent_steps[store.key()]
        .progress
        .get("typed_failure")
        .is_none()
    );
    for _ in 0..2 {
        assert_eq!(
            client
                .answer(&store, "legacy prompt".into(), Duration::from_secs(30))
                .await
                .unwrap(),
            r#"{"legacy":true}"#
        );
    }
    let mut state: RunState = serde_json::from_value(store.load().await.unwrap().unwrap()).unwrap();
    state.answer = Some("replacement".into());
    assert!(matches!(
        store.save(serde_json::to_value(state).unwrap()).await,
        Err(ReviewError::PersistenceFence)
    ));
    assert!(transport.calls.lock().await.is_empty());
}
#[tokio::test]
async fn legacy_terminal_runstate_replays_through_memory_store() {
    replay_pre_byok_terminal_state(None).await;
}
#[tokio::test]
async fn legacy_terminal_runstate_replays_through_mongo_store() {
    let Ok(uri) = std::env::var("WISHPOOL_TEST_MONGODB_URI") else {
        return;
    };
    let name = format!(
        "wishpool_test_legacy_{}",
        crate::auth::random_token()
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(16)
            .collect::<String>()
    );
    let store = crate::store::MongoStore::connect(&uri, &name)
        .await
        .unwrap();
    replay_pre_byok_terminal_state(Some(Arc::new(store))).await;
    mongodb::Client::with_uri_str(&uri)
        .await
        .unwrap()
        .database(&name)
        .drop()
        .await
        .unwrap();
}

#[tokio::test]
async fn donation_attribution_follows_operator_payer_and_is_idempotent() {
    use mathesis_review::cma::{FailureKind, RunStore};
    let (w, _, _) = author_cma_worker().await;
    let donor = w
        .worker
        .app
        .ensure_service_account(&"supporter".into(), "Supporter", Role::Endorser)
        .await
        .unwrap();
    w.worker
        .app
        .create_or_renew_donation(&donor.person, 10000, "model")
        .await
        .unwrap();
    let job = w.job().await;
    for label in ["r1:formal", "target:C1:a0", "target:C1:a0:faithfulness"] {
        w.worker
            .agent_store(&job, &w.paper, label)
            .execution()
            .await
            .unwrap();
    }
    assert!(
        w.worker
            .app
            .donation(&donor)
            .await
            .unwrap()
            .unwrap()
            .runs
            .is_empty()
    );
    let target = w.worker.agent_store(&job, &w.paper, "target:C1:a0");
    target.execution().await.unwrap();
    assert!(target.fallback(FailureKind::Quota).await.unwrap());
    for _ in 0..2 {
        target.execution().await.unwrap();
    }
    let venue = w
        .worker
        .agent_store(&job, &w.paper, "agent-attempt:C1:digest");
    for _ in 0..2 {
        venue.execution().await.unwrap();
    }
    w.worker
        .agent_store(&job, &w.paper, "error-report:report1")
        .execution()
        .await
        .unwrap();
    let grant = w.worker.app.donation(&donor).await.unwrap().unwrap();
    assert_eq!(grant.runs.len(), 2);
    assert!(grant.runs.contains_key(target.key()));
    assert!(grant.runs.contains_key(venue.key()));
    assert_eq!(grant.used, 0);
    assert!(grant.runs.values().all(|r| r.usage.is_none()));
}

#[tokio::test]
async fn review_only_worker_delivers_private_letter_without_publication_or_lean_probe() {
    use mathesis_core::{
        model::{SubmissionPurpose, Visibility},
        ports::SubmissionStore,
    };
    let mut w = World::new(vec![completed("reject")], true).await;
    let mut paper = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    let revision = paper.revision;
    paper.purpose = SubmissionPurpose::ReviewOnly;
    paper.analysis_visibility = Visibility::Private;
    paper.decision = None;
    paper.revision += 1;
    w.stores.replace(&paper, revision).await.unwrap();
    let advisor = Arc::new(FakeAdvisor {
        known: true,
        ..Default::default()
    });
    w.worker.advisor = Some(advisor.clone());
    w.run_round().await;
    let paper = w.worker.app.submission(&w.author, &paper.id).await.unwrap();
    assert_eq!(paper.status, SubmissionStatus::ReviewComplete);
    assert!(paper.decision.is_none());
    assert_eq!(paper.analysis_visibility, Visibility::Private);
    let file = w.file().await;
    assert!(file.current().unwrap().is_settled());
    assert_eq!(file.letters.len(), 1);
    assert!(
        file.letters[0]
            .body
            .starts_with("Your private Orchestra review is complete.")
    );
    assert_eq!(advisor.advice.load(Ordering::SeqCst), 1);
    assert_eq!(advisor.letters.load(Ordering::SeqCst), 1);
    assert_eq!(w.formalizer.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn private_followup_delivers_a_bound_conclusion_without_rewriting_the_original_audit() {
    use mathesis_core::{
        model::{NewFindingResponse, ResponseDisposition, SubmissionPurpose, Visibility},
        ports::SubmissionStore,
    };
    let mut w = World::new(
        vec![completed("major_revision"), completed("minor_revision")],
        true,
    )
    .await;
    let mut paper = w
        .worker
        .app
        .submission(&w.author, &w.paper.id)
        .await
        .unwrap();
    let revision = paper.revision;
    paper.purpose = SubmissionPurpose::ReviewOnly;
    paper.analysis_visibility = Visibility::Private;
    paper.revision += 1;
    w.stores.replace(&paper, revision).await.unwrap();
    w.worker.advisor = Some(Arc::new(FakeAdvisor {
        followup_concern: true,
        ..Default::default()
    }));
    w.run_round().await;
    let original = w.file().await.current().unwrap().audit.clone();
    assert_eq!(original.done().unwrap().concerns.len(), 1);
    let response = NewFindingResponse {
        round: 1,
        concern: 0,
        version: 1,
        claims_revision: paper.claims_revision,
        body: "The convention is explicit in the quantified domain; please recheck it.".into(),
    };
    let view = w
        .worker
        .app
        .respond_to_finding(&w.author, &paper.id, response.clone())
        .await
        .unwrap();
    let response_id = &view.finding_responses[0].id;
    w.run_round().await;
    let report = w
        .worker
        .app
        .review_report(&w.author, &paper.id)
        .await
        .unwrap();
    assert_eq!(report.review.letters.len(), 2);
    assert_eq!(report.review.rounds.len(), 2);
    let result = &report.review.rounds[1]
        .audit
        .done()
        .unwrap()
        .response_assessments[0];
    assert_eq!(&result.response_id, response_id);
    assert_eq!(result.status, ResponseDisposition::Addressed);
    assert_eq!(w.file().await.rounds[0].audit, original);
    assert!(report.markdown().contains("matches the quantified domain"));
    assert!(report.review.letters.iter().all(|l| {
        l.body
            .starts_with("Your private Orchestra review is complete.")
    }));
    assert!(
        report
            .review
            .rounds
            .iter()
            .all(|r| r.advice.is_none() && r.letter.is_none())
    );
    assert!(
        w.worker
            .app
            .submission(&w.author, &paper.id)
            .await
            .unwrap()
            .decision
            .is_none()
    );
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 2);
    w.worker
        .app
        .respond_to_finding(&w.author, &paper.id, response)
        .await
        .unwrap();
    assert!(!w.worker.referee_job(&w.job().await).await.unwrap());
    assert_eq!(w.oracle.submitted.load(Ordering::SeqCst), 2);
}
