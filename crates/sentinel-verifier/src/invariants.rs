//! Built-in invariant encodings for the Z3 engine.

/// The result of checking a single invariant.
#[derive(Debug, Clone)]
pub enum VerificationResult {
    /// Invariant holds — no counterexample exists within modelled bounds.
    Holds,
    /// Invariant violated — counterexample model included.
    Violated { counterexample: String },
    /// Solver timed out or hit resource limits.
    Unknown,
}

/// Identifies which built-in invariant to check.
#[derive(Debug, Clone, PartialEq)]
pub enum Invariant {
    BalanceConservation,
    AccessControl,
    NoOverflow,
}

impl Invariant {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "balance_conservation" => Some(Self::BalanceConservation),
            "access_control" => Some(Self::AccessControl),
            "no_overflow" => Some(Self::NoOverflow),
            _ => None,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::BalanceConservation => "Sum of all account balances equals total supply",
            Self::AccessControl => "Privileged functions revert for non-owner callers",
            Self::NoOverflow => "All arithmetic operations are overflow-free",
        }
    }
}
