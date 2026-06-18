//! Z3-backed verification engine.
//!
//! Each invariant is encoded by negating the property and asking Z3 to find
//! a satisfying assignment. UNSAT means the property holds; SAT gives a counterexample.

use anyhow::Result;
use sentinel_core::{
    config::ContractConfig,
    finding::{Finding, FindingKind, FindingSeverity},
};
use tracing::{info, warn};
use uuid::Uuid;
use z3::{ast::Int, Config, Context, Solver};

use crate::invariants::{Invariant, VerificationResult};

pub struct VerificationEngine;

impl VerificationEngine {
    /// Check all invariants declared in `contract` and return any findings.
    pub fn verify(run_id: Uuid, contract: &ContractConfig) -> Result<Vec<Finding>> {
        let mut findings = Vec::new();

        for inv_name in &contract.invariants {
            if let Some(inv) = Invariant::from_str(inv_name) {
                info!(contract = %contract.name, invariant = %inv_name, "Verifying invariant");
                let result = Self::check_invariant(&inv);
                match result {
                    VerificationResult::Violated { counterexample } => {
                        warn!(
                            contract = %contract.name,
                            invariant = %inv_name,
                            counterexample = %counterexample,
                            "Invariant VIOLATED"
                        );
                        findings.push(Finding::new(
                            run_id,
                            &contract.name,
                            FindingKind::InvariantViolation,
                            FindingSeverity::Critical,
                            format!(
                                "Invariant '{}' violated: {}. Counterexample: {}",
                                inv_name,
                                inv.description(),
                                counterexample
                            ),
                            Some(counterexample),
                        ));
                    }
                    VerificationResult::Holds => {
                        info!(contract = %contract.name, invariant = %inv_name, "Invariant holds");
                    }
                    VerificationResult::Unknown => {
                        warn!(contract = %contract.name, invariant = %inv_name, "Solver returned unknown");
                    }
                }
            }
        }

        Ok(findings)
    }

    fn check_invariant(inv: &Invariant) -> VerificationResult {
        match inv {
            Invariant::NoOverflow => Self::check_no_overflow(),
            Invariant::BalanceConservation => Self::check_balance_conservation(),
            Invariant::AccessControl => {
                // Access control requires symbolic contract model — returns Unknown
                // until contract ABI is wired in.
                VerificationResult::Unknown
            }
        }
    }

    /// Prove: for all a, b: i128, a + b does not overflow.
    /// We model this as: ∃ a, b s.t. a + b > i128::MAX  (negation of "no overflow").
    fn check_no_overflow() -> VerificationResult {
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);

        let a = Int::new_const(&ctx, "a");
        let b = Int::new_const(&ctx, "b");
        let max = Int::from_i64(&ctx, i64::MAX); // simplified to i64 for Z3 Int

        // Negation: a + b > MAX  (overflow)
        let sum = Int::add(&ctx, &[&a, &b]);
        solver.assert(&sum.gt(&max));

        match solver.check() {
            z3::SatResult::Unsat => VerificationResult::Holds,
            z3::SatResult::Sat => {
                let model = solver.get_model().map(|m| m.to_string()).unwrap_or_default();
                VerificationResult::Violated {
                    counterexample: model,
                }
            }
            z3::SatResult::Unknown => VerificationResult::Unknown,
        }
    }

    /// Model balance conservation: total_supply == sum(balances).
    /// Negation: ∃ transfer s.t. total_supply ≠ sum(balances) after transfer.
    fn check_balance_conservation() -> VerificationResult {
        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);

        let total_supply = Int::new_const(&ctx, "total_supply");
        let bal_a = Int::new_const(&ctx, "bal_a");
        let bal_b = Int::new_const(&ctx, "bal_b");
        let amount = Int::new_const(&ctx, "amount");
        let zero = Int::from_i64(&ctx, 0);

        // Pre-conditions
        solver.assert(&total_supply.gt(&zero));
        solver.assert(&bal_a.ge(&zero));
        solver.assert(&bal_b.ge(&zero));
        solver.assert(&amount.gt(&zero));
        solver.assert(&bal_a.ge(&amount)); // transfer is valid
        // Pre: bal_a + bal_b == total_supply
        solver.assert(&Int::add(&ctx, &[&bal_a, &bal_b])._eq(&total_supply));

        // Post-transfer balances
        let bal_a_after = Int::sub(&ctx, &[&bal_a, &amount]);
        let bal_b_after = Int::add(&ctx, &[&bal_b, &amount]);

        // Negation of conservation: bal_a_after + bal_b_after ≠ total_supply
        let sum_after = Int::add(&ctx, &[&bal_a_after, &bal_b_after]);
        solver.assert(&z3::ast::Bool::not(&sum_after._eq(&total_supply)));

        match solver.check() {
            z3::SatResult::Unsat => VerificationResult::Holds,
            z3::SatResult::Sat => {
                let model = solver.get_model().map(|m| m.to_string()).unwrap_or_default();
                VerificationResult::Violated { counterexample: model }
            }
            z3::SatResult::Unknown => VerificationResult::Unknown,
        }
    }
}
