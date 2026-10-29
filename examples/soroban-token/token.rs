// Deliberately paired regression fixture, not a production token implementation.
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub enum Key {
    Balance(Address),
    Supply,
}

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn initialize(env: Env, owner: Address, supply: i128) {
        owner.require_auth();
        assert!(!env.storage().instance().has(&Key::Supply));
        assert!(supply > 0);
        env.storage().instance().set(&Key::Supply, &supply);
        env.storage().persistent().set(&Key::Balance(owner), &supply);
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0);
        let debit = Self::balance(env.clone(), from.clone());
        assert!(debit >= amount);
        // The vulnerable version reads both balances before writing to the same key.
        // A self-transfer then mints `amount` without changing total supply.
        #[cfg(not(feature = "vulnerable"))]
        if from == to {
            return;
        }
        let credit = Self::balance(env.clone(), to.clone());
        let after_credit = credit.checked_add(amount).expect("balance overflow");
        env.storage().persistent().set(&Key::Balance(from), &(debit - amount));
        env.storage().persistent().set(&Key::Balance(to), &after_credit);
    }

    pub fn balance(env: Env, account: Address) -> i128 {
        env.storage().persistent().get(&Key::Balance(account)).unwrap_or(0)
    }

    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().get(&Key::Supply).unwrap_or(0)
    }
}

#[cfg(test)]
mod sentinel_invariants {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn balance_conservation() {
        for supply in [1_i128, 2, 100, 1_000_000] {
            let env = Env::default();
            let id = env.register(Token, ());
            let client = TokenClient::new(&env, &id);
            let owner = Address::generate(&env);
            let recipient = Address::generate(&env);
            client.mock_all_auths().initialize(&owner, &supply);
            // Include aliasing of source and destination: the original missed edge case.
            for amount in [1, supply] {
                client.mock_all_auths().transfer(&owner, &owner, &amount);
                assert_eq!(client.balance(&owner), supply, "self-transfer changed supply; amount={amount}");
            }
            client.mock_all_auths().transfer(&owner, &recipient, &supply);
            assert_eq!(client.balance(&owner) + client.balance(&recipient), client.total_supply());
        }
    }

    #[test]
    fn access_control() {
        let env = Env::default();
        let id = env.register(Token, ());
        let client = TokenClient::new(&env, &id);
        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        client.mock_all_auths().initialize(&owner, &100);
        // No mocked authorization for this invocation.
        assert!(client.try_transfer(&owner, &attacker, &1).is_err());
        assert_eq!(client.balance(&owner), 100);
        assert_eq!(client.balance(&attacker), 0);
    }

    #[test]
    fn no_overflow() {
        let env = Env::default();
        let id = env.register(Token, ());
        let client = TokenClient::new(&env, &id);
        let owner = Address::generate(&env);
        let recipient = Address::generate(&env);
        client.mock_all_auths().initialize(&owner, &i128::MAX);
        client.mock_all_auths().transfer(&owner, &recipient, &i128::MAX);
        client.mock_all_auths().transfer(&recipient, &owner, &i128::MAX);
        assert_eq!(client.balance(&owner), i128::MAX);
        for invalid in [0, -1, i128::MIN] {
            assert!(client.mock_all_auths().try_transfer(&owner, &recipient, &invalid).is_err());
        }
        assert_eq!(client.balance(&owner), i128::MAX);
        assert_eq!(client.balance(&recipient), 0);
    }
}
