use std::{
    collections::{BTreeSet, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use tokio::sync::Mutex;
use wishpool_core::{
    app::{App, Upload},
    memory::MemoryStores,
    model::{
        AiDisclosure, AiUse, Caller, ClaimConfirmation, FeedbackLetter, NewLetter, NewPaper,
        Recommendation, ReviewerIdentity, Role, StepState, Submission,
    },
    policy::Policy,
    ports::{JobKind, JobLease, SystemClock},
};
use wishpool_review::{
    ReviewResult,
    advisor::{Advisor, AdvisorInput},
    lean::{CheckedFile, FormalInput, FormalOut, Formalizer},
    oracle::{Oracle, OracleRequest, OracleStatus, OracleSubmitted},
    referee_prompts::{AdviceOut, AuditOut, AuditedClaimOut, FormalizationOut, LetterOut},
};

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
                    theorem: Some("Wishpool.C1.main".into()),
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
            verdict: "major_revision".into(),
            summary: "The argument is correct and carries new content.".into(),
            claims: input
                .statements
                .iter()
                .map(|c| AuditedClaimOut {
                    claim: c.id.clone(),
                    conjecture: (input.kind == "conjecture" && !c.proved).then(|| {
                        wishpool_review::referee_prompts::ConjectureOut {
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
        Self::new_kind(outcomes, pdf, wishpool_core::model::SubmissionKind::Paper).await
    }
    async fn new_kind(
        outcomes: Vec<ReviewResult<OracleStatus>>,
        pdf: bool,
        kind: wishpool_core::model::SubmissionKind,
    ) -> Self {
        Self::new_kind_with_cma(outcomes, pdf, kind, false).await
    }
    async fn new_kind_with_cma(
        outcomes: Vec<ReviewResult<OracleStatus>>,
        pdf: bool,
        kind: wishpool_core::model::SubmissionKind,
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
        wishpool_core::ports::ReviewQueue::enqueue(&*self.stores, &self.paper.id, JobKind::Referee)
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

    async fn file(&self) -> wishpool_core::model::RefereeFile {
        wishpool_core::ports::RefereeStore::get(&*self.stores, &self.paper.id)
            .await
            .unwrap()
            .unwrap_or_else(|| wishpool_core::model::RefereeFile::new(self.paper.id.clone()))
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
        wishpool_core::model::ProbeOutcome::Compiled
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
            wishpool_core::model::Stage::Literature,
            wishpool_core::model::ReportDraft {
                outcome: wishpool_core::model::Outcome::Fail {
                    reason: wishpool_core::model::RejectReason::OutOfScope {
                        detail: "Outside the venue's field".into(),
                    },
                },
                summary: "Outside scope".into(),
                payload: wishpool_core::model::StagePayload::Literature {
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
        wishpool_core::judgement::Standing::Corroborated
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
            .latest_report(wishpool_core::model::Stage::Escape)
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
    async fn claim(&self) -> wishpool_core::CoreResult<Option<LeasedJob>> {
        self.jobs.claim().await
    }
    async fn renew(&self, job: &LeasedJob) -> wishpool_core::CoreResult<bool> {
        self.renewals.fetch_add(1, Ordering::SeqCst);
        if tokio::time::Instant::now() >= self.lose_at {
            Ok(false)
        } else {
            self.jobs.renew(job).await
        }
    }
    async fn complete(&self, _job: &LeasedJob) -> wishpool_core::CoreResult<()> {
        self.settlements.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn defer(&self, _job: &LeasedJob, _delay: Duration) -> wishpool_core::CoreResult<()> {
        self.settlements.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    async fn retry(&self, _job: &LeasedJob, _error: &str) -> wishpool_core::CoreResult<()> {
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
    use wishpool_core::model::{
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
    use wishpool_core::{
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
    let mut primary = wishpool_core::ports::RefereeStore::get(&*w.stores, &w.paper.id)
        .await
        .unwrap()
        .unwrap();
    let expected = primary.revision;
    primary.rounds.clear();
    primary.revision += 1;
    wishpool_core::ports::RefereeStore::replace(&*w.stores, &primary, expected)
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
impl wishpool_review::cma::Transport for ManagedTransport {
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
        _key: Option<&str>,
    ) -> ReviewResult<wishpool_review::cma::TransportReply> {
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
        } else {
            json!(null)
        };
        Ok(wishpool_review::cma::TransportReply {
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
    let client = Arc::new(wishpool_review::cma::Client {
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
    w.worker.advisor = Some(Arc::new(wishpool_review::cma::CmaAdvisor {
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
            wishpool_core::model::CmaPayer::Operator
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
    use wishpool_review::cma::{RunState, RunStore};
    let mut w = World::new(vec![], true).await;
    let transport = Arc::new(ManagedTransport {
        created: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
        answers: Mutex::new(Default::default()),
    });
    w.worker.advisor = Some(Arc::new(wishpool_review::cma::CmaAdvisor {
        client: Arc::new(wishpool_review::cma::Client {
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
impl wishpool_core::ports::CmaSetup for TestCmaSetup {
    fn enabled(&self) -> bool {
        true
    }
    async fn validate(
        &self,
        _: &wishpool_core::model::CmaBinding,
        _: &str,
    ) -> wishpool_core::CoreResult<wishpool_core::model::CmaWorkspaceCheck> {
        Err(wishpool_core::CoreError::Unavailable("not used".into()))
    }
}
struct TestCmaToken(String);
#[async_trait]
impl wishpool_review::cma::TokenSource for TestCmaToken {
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
    use wishpool_review::cma::Transport;
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
        ["resource", "allowed_service_ids", "cnf", "act", "delegated"]
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
            return (
                axum::http::StatusCode::BAD_GATEWAY,
                axum::Json(serde_json::json!({"detail":token})),
            )
                .into_response();
        }
        return (axum::http::StatusCode::from_u16(status).unwrap(), body).into_response();
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
async fn author_cma_worker() -> (World, Arc<AccountCma>, Arc<wishpool_review::cma::Client>) {
    use base64::Engine as _;
    use wishpool_core::{
        model::{CmaBinding, CmaStatus},
        ports::CmaBindingStore,
    };
    let mut w = World::new_kind_with_cma(
        vec![],
        true,
        wishpool_core::model::SubmissionKind::Paper,
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
            &serde_json::json!({"sub":subject,"exp":chrono::Utc::now().timestamp()+3600}),
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
    let operator = Arc::new(wishpool_review::cma::Client {
        transport: Arc::new(wishpool_review::cma::HttpTransport {
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
    w.worker.advisor = Some(Arc::new(wishpool_review::cma::CmaAdvisor {
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
    assert_eq!(file.agent_steps.len(), 6);
    for step in file.agent_steps.values() {
        let attempt = &step.cma_attempts[0];
        assert_eq!(attempt.payer, wishpool_core::model::CmaPayer::Author);
        assert_eq!(attempt.workspace, "wks_author");
        assert_eq!(attempt.outcome, wishpool_core::model::CmaOutcome::Succeeded);
        assert_ne!(
            attempt.identity.kind,
            wishpool_core::model::CmaStepKind::LegacyUnknown
        );
    }
    assert_eq!(mock.author.created.load(Ordering::SeqCst), 6);
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
        wishpool_core::model::CmaPayer::Author
    );
    assert_eq!(
        step.cma_attempts[0].failure,
        Some(wishpool_core::model::CmaFailure::Quota)
    );
    assert_eq!(
        step.cma_attempts[1].payer,
        wishpool_core::model::CmaPayer::Operator
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
    use wishpool_review::cma::{FailureKind, RunState, RunStore};
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
        assert!(
            tokio::time::timeout(
                Duration::from_millis(60),
                client.answer(
                    &store,
                    "Every gap is finite. You are advising".into(),
                    Duration::from_secs(30)
                )
            )
            .await
            .is_err()
        );
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
            wishpool_core::model::CmaPayer::Author
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
    let attempt = w
        .worker
        .app
        .admit_cma_attempt(
            &w.worker.auditor_account,
            &w.paper.id,
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
            wishpool_review::cma::FailureKind::Auth
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
    let attempt = w
        .worker
        .app
        .admit_cma_attempt(
            &w.worker.auditor_account,
            &w.paper.id,
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
        assert!(matches!(error, wishpool_core::CoreError::Unavailable(_)));
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
        wishpool_core::model::CmaStatus::Ready
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
    ] {
        let (w, mock, client) = author_cma_worker().await;
        let mut claims =
            serde_json::json!({"sub":"author","exp":chrono::Utc::now().timestamp()+3600});
        claims[restriction] = serde_json::json!(true);
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
        assert_eq!(attempts[0].payer, wishpool_core::model::CmaPayer::Author);
        assert_eq!(
            attempts[0].failure,
            Some(wishpool_core::model::CmaFailure::Auth)
        );
        assert_eq!(attempts[1].payer, wishpool_core::model::CmaPayer::Operator);
        assert_eq!(mock.author.created.load(Ordering::SeqCst), 0);
        assert_eq!(mock.operator.created.load(Ordering::SeqCst), 1);
    }
}

struct HttpCmaFormalizer {
    client: Arc<wishpool_review::cma::Client>,
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
        store: &dyn wishpool_review::cma::RunStore,
    ) -> ReviewResult<FormalOut> {
        // Replace only the credential-free local checker; drive actual worker dispatch and CMA transport.
        self.client
            .answer(
                store,
                wishpool_review::referee_prompts::formalize(input, "test pinned toolchain"),
                Duration::from_secs(30),
            )
            .await?;
        self.checker.formalize(input, work).await
    }
}

#[tokio::test]
async fn real_worker_dispatches_all_six_cma_steps_for_author_and_operator_and_restarts_uncertain_creation()
 {
    use wishpool_core::model::CmaStepKind;
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
        assert!(
            tokio::time::timeout(Duration::from_millis(100), w.worker.referee_job(&job))
                .await
                .is_err()
        );
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
        wishpool_core::ports::ReviewQueue::enqueue(
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
                    wishpool_core::model::CmaPayer::Author
                } else {
                    wishpool_core::model::CmaPayer::Operator
                }
            );
            assert_eq!(
                step.cma_attempts[0].outcome,
                wishpool_core::model::CmaOutcome::Succeeded
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
            6
        );
        assert_eq!(
            if author_pays {
                mock.operator.created.load(Ordering::SeqCst)
            } else {
                mock.author.created.load(Ordering::SeqCst)
            },
            0
        );
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
        let error = tokio::time::timeout(
            Duration::from_millis(60),
            client.answer(
                &store,
                "Every gap is finite. You are advising".into(),
                Duration::from_secs(30),
            ),
        )
        .await
        .unwrap_err();
        tracing::warn!(%error,"refresh retry");
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
        Some(wishpool_core::model::CmaFailure::Auth)
    );
    assert_eq!(attempts[1].payer, wishpool_core::model::CmaPayer::Operator);
    assert_eq!(mock.author.created.load(Ordering::SeqCst), 0);
    assert_eq!(mock.operator.created.load(Ordering::SeqCst), 1);
}
