//! Z3-backed verification engine.
//!
//! Each invariant is encoded by negating the property and asking Z3 to find
//! a satisfying assignment. UNSAT means the property holds; SAT gives a counterexample.

use anyhow::Result;
use sentinel_core::{config::ContractConfig, finding::Finding};
use uuid::Uuid;
use z3::{
    ast::{Ast, Int},
    Config, Context, Solver,
};

use crate::invariants::{Invariant, VerificationResult};

pub struct VerificationEngine;

impl VerificationEngine {
    /// Template equations do not establish properties of the requested contract.
    pub fn verify(_run_id: Uuid, contract: &ContractConfig) -> Result<Vec<Finding>> {
        anyhow::bail!(
            "verification unavailable for '{}': no contract-derived symbolic model",
            contract.name
        )
    }

    /// Educational template evaluation; never a proof about contract code.
    pub fn check_template(inv: &Invariant) -> VerificationResult {
        Self::check_invariant(inv)
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
        let mut cfg = Config::new();
        cfg.set_timeout_msec(5000);
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);

        let a = Int::new_const(&ctx, "a");
        let b = Int::new_const(&ctx, "b");
        let max = Int::from_str(&ctx, &i128::MAX.to_string()).expect("valid i128 bound");
        let min = Int::from_str(&ctx, &i128::MIN.to_string()).expect("valid i128 bound");
        for operand in [&a, &b] {
            solver.assert(&operand.ge(&min));
            solver.assert(&operand.le(&max));
        }
        // Both overflow and underflow, with operands constrained to signed i128.
        let sum = Int::add(&ctx, &[&a, &b]);
        solver.assert(&z3::ast::Bool::or(&ctx, &[&sum.gt(&max), &sum.lt(&min)]));

        match solver.check() {
            z3::SatResult::Unsat => VerificationResult::Holds,
            z3::SatResult::Sat => {
                let model = solver
                    .get_model()
                    .map(|m| m.to_string())
                    .unwrap_or_default();
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
        let mut cfg = Config::new();
        cfg.set_timeout_msec(5000);
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
                let model = solver
                    .get_model()
                    .map(|m| m.to_string())
                    .unwrap_or_default();
                VerificationResult::Violated {
                    counterexample: model,
                }
            }
            z3::SatResult::Unknown => VerificationResult::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_to_claim_a_contract_proof() {
        let c = ContractConfig {
            name: "token".into(),
            path: "token".into(),
            invariants: vec!["balance_conservation".into()],
            fuzz_timeout_secs: 1,
            fuzz_iterations: 0,
        };
        assert!(VerificationEngine::verify(Uuid::new_v4(), &c).is_err());
    }
    #[test]
    fn templates_are_separate_from_contract_proofs() {
        assert!(matches!(
            VerificationEngine::check_template(&Invariant::BalanceConservation),
            VerificationResult::Holds
        ));
        assert!(matches!(
            VerificationEngine::check_template(&Invariant::NoOverflow),
            VerificationResult::Violated { .. }
        ));
        assert!(matches!(
            VerificationEngine::check_template(&Invariant::AccessControl),
            VerificationResult::Unknown
        ));
    }
}
