//! sentinel-verifier — lightweight formal verification via Z3 SMT solver.
//!
//! For each invariant template we encode the property as a Z3 formula and
//! check satisfiability of its negation. If SAT → counterexample found (violation).
//! If UNSAT → invariant holds for all inputs within the modelled bounds.

pub mod engine;
pub mod invariants;
