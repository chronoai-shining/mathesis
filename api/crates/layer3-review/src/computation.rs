//! Bounded exact finite arithmetic. Inputs are data; no uploaded code is run.
//! A replay proves only the specified matrix calculation, never manuscript
//! faithfulness, general novelty, formal coverage or a Lean theorem.
use serde::{Deserialize, Serialize};

pub const PROTOCOL: &str = "mathesis.rational-projection-amgm.v1";
pub const MAX_BYTES: usize = 16_384;
const MAX_DIMENSION: usize = 6;
const MAX_ENTRY: i64 = 100;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Certificate {
    protocol: String,
    vectors: [Vec<i64>; 3],
    witness: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayState {
    Counterexample,
    NotCounterexample,
    InvalidInput,
    ArithmeticLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replay {
    pub protocol: String,
    pub state: ReplayState,
    pub dimension: Option<usize>,
    pub numerator: Option<String>,
    pub denominator: Option<String>,
    pub explanation: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct Rational {
    numerator: i128,
    denominator: i128,
}
const ZERO: Rational = Rational {
    numerator: 0,
    denominator: 1,
};
fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
impl Rational {
    fn new(numerator: i128, denominator: i128) -> Result<Self, ()> {
        if denominator <= 0 {
            return Err(());
        }
        let divisor = gcd(numerator.checked_abs().ok_or(())?, denominator);
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }
    fn add(self, other: Self) -> Result<Self, ()> {
        let divisor = gcd(self.denominator, other.denominator);
        let left = other.denominator / divisor;
        let right = self.denominator / divisor;
        Self::new(
            self.numerator
                .checked_mul(left)
                .ok_or(())?
                .checked_add(other.numerator.checked_mul(right).ok_or(())?)
                .ok_or(())?,
            self.denominator.checked_mul(left).ok_or(())?,
        )
    }
    fn multiply(self, other: Self) -> Result<Self, ()> {
        // Cross-cancel before multiplication; overflow stays an explicit limit.
        let left = gcd(self.numerator.checked_abs().ok_or(())?, other.denominator);
        let right = gcd(other.numerator.checked_abs().ok_or(())?, self.denominator);
        Self::new(
            (self.numerator / left)
                .checked_mul(other.numerator / right)
                .ok_or(())?,
            (self.denominator / right)
                .checked_mul(other.denominator / left)
                .ok_or(())?,
        )
    }
}
type Matrix = Vec<Vec<Rational>>;
fn product(a: &Matrix, b: &Matrix) -> Result<Matrix, ()> {
    (0..a.len())
        .map(|i| {
            (0..a.len())
                .map(|j| (0..a.len()).try_fold(ZERO, |sum, k| sum.add(a[i][k].multiply(b[k][j])?)))
                .collect()
        })
        .collect()
}
fn calculation(certificate: &Certificate) -> Result<Rational, ()> {
    let dimension = certificate.witness.len();
    let mut projections = Vec::new();
    for vector in &certificate.vectors {
        let norm: i128 = vector.iter().map(|v| i128::from(*v).pow(2)).sum();
        let matrix: Matrix = vector
            .iter()
            .map(|a| {
                vector
                    .iter()
                    .map(|b| Rational::new(i128::from(*a) * i128::from(*b), norm))
                    .collect()
            })
            .collect::<Result<_, _>>()?;
        // Check the actual reconstructed matrices, not an input assertion.
        if product(&matrix, &matrix)? != matrix
            || (0..dimension).any(|i| (0..dimension).any(|j| matrix[i][j] != matrix[j][i]))
        {
            return Err(());
        }
        projections.push(matrix);
    }
    let third = Rational::new(1, 3)?;
    let negative_sixth = Rational::new(-1, 6)?;
    let mean: Matrix = (0..dimension)
        .map(|i| {
            (0..dimension)
                .map(|j| {
                    projections
                        .iter()
                        .try_fold(ZERO, |sum, p| sum.add(p[i][j]))?
                        .multiply(third)
                })
                .collect()
        })
        .collect::<Result<_, _>>()?;
    let mut difference = product(&product(&mean, &mean)?, &mean)?;
    for [a, b, c] in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let term = product(&product(&projections[a], &projections[b])?, &projections[c])?;
        for i in 0..dimension {
            for j in 0..dimension {
                difference[i][j] = difference[i][j].add(term[i][j].multiply(negative_sixth)?)?;
            }
        }
    }
    (0..dimension).try_fold(ZERO, |sum, i| {
        (0..dimension).try_fold(sum, |sum, j| {
            sum.add(difference[i][j].multiply(Rational::new(
                i128::from(certificate.witness[i]) * i128::from(certificate.witness[j]),
                1,
            )?)?)
        })
    })
}

pub fn replay(bytes: &[u8]) -> Replay {
    let invalid = || {
        Replay { protocol: PROTOCOL.into(), state: ReplayState::InvalidInput,
        dimension: None, numerator: None, denominator: None,
        explanation: "Expected three nonzero integer vectors and one witness of the same dimension 1–6, entries in [-100,100], under the exact recognized protocol; at most 16384 input bytes. No code is executed.".into() }
    };
    if bytes.len() > MAX_BYTES {
        return invalid();
    }
    let Ok(certificate) = serde_json::from_slice::<Certificate>(bytes) else {
        return invalid();
    };
    let dimension = certificate.witness.len();
    if certificate.protocol != PROTOCOL
        || !(1..=MAX_DIMENSION).contains(&dimension)
        || certificate
            .vectors
            .iter()
            .any(|v| v.len() != dimension || v.iter().all(|x| *x == 0))
        || certificate
            .vectors
            .iter()
            .flatten()
            .chain(&certificate.witness)
            .any(|x| !(-MAX_ENTRY..=MAX_ENTRY).contains(x))
    {
        return invalid();
    }
    match calculation(&certificate) {
        Ok(value) => Replay { protocol: PROTOCOL.into(),
            state: if value.numerator < 0 { ReplayState::Counterexample } else { ReplayState::NotCounterexample },
            dimension: Some(dimension), numerator: Some(value.numerator.to_string()), denominator: Some(value.denominator.to_string()),
            explanation: "Exact xᵀDx for rank-one orthogonal projections Pᵢ=vᵢvᵢᵀ/(vᵢᵀvᵢ), D=((P₁+P₂+P₃)/3)³−Σσ Pσ(1)Pσ(2)Pσ(3)/6. All six permutations retain multiplication order. A negative value refutes positive semidefiniteness of this D; a nonnegative value proves no universal inequality. Matching this assertion to a manuscript requires a separate source audit.".into() },
        Err(()) => Replay { protocol: PROTOCOL.into(), state: ReplayState::ArithmeticLimit,
            dimension: Some(dimension), numerator: None, denominator: None,
            explanation: "Checked exact rational arithmetic exceeded its representable range. No mathematical result is claimed.".into() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> serde_json::Value {
        serde_json::json!({"protocol":PROTOCOL,"vectors":[[1,-1,-1],[-1,1,-1],[-1,-1,1]],"witness":[1,1,1]})
    }
    fn run(value: &serde_json::Value) -> Replay {
        replay(&serde_json::to_vec(value).unwrap())
    }
    #[test]
    fn reproduces_the_exact_known_counterexample_and_rejects_a_false_witness() {
        let mut certificate = input();
        let result = run(&certificate);
        assert_eq!(result.state, ReplayState::Counterexample);
        assert_eq!(result.numerator.as_deref(), Some("-8"));
        assert_eq!(result.denominator.as_deref(), Some("243"));
        certificate["witness"] = serde_json::json!([0, 0, 0]);
        assert_eq!(run(&certificate).state, ReplayState::NotCounterexample);
        certificate["vectors"] = serde_json::json!([[1, 0, 0], [1, 0, 0], [1, 0, 0]]);
        certificate["witness"] = serde_json::json!([1, 1, 1]);
        assert_eq!(run(&certificate).numerator.as_deref(), Some("0"));
    }
    #[test]
    fn protocol_shape_and_resource_limits_are_enforced() {
        for (key, value) in [
            ("protocol", serde_json::json!("ordinary-lean-proof")),
            (
                "vectors",
                serde_json::json!([[0, 0, 0], [-1, 1, -1], [-1, -1, 1]]),
            ),
            (
                "vectors",
                serde_json::json!([[1, -1], [-1, 1, -1], [-1, -1, 1]]),
            ),
            ("witness", serde_json::json!([101, 1, 1])),
            ("numerator", serde_json::json!(-8)),
        ] {
            let mut certificate = input();
            certificate[key] = value;
            assert_eq!(run(&certificate).state, ReplayState::InvalidInput);
        }
        assert_eq!(
            replay(&vec![b' '; MAX_BYTES + 1]).state,
            ReplayState::InvalidInput
        );
    }
    #[test]
    fn arithmetic_overflow_is_never_wrapped_or_saturated() {
        assert!(
            Rational::new(i128::MAX, 1)
                .unwrap()
                .add(Rational::new(1, 1).unwrap())
                .is_err()
        );
        assert!(Rational::new(i128::MIN, 1).is_err());
        assert!(
            Rational::new(i128::MAX, 1)
                .unwrap()
                .multiply(Rational::new(2, 1).unwrap())
                .is_err()
        );
    }
}
