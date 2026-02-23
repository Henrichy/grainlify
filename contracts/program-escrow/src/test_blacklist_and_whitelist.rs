#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String,
};

// ============================================================================
// Test Helpers
// ============================================================================

/// Creates a default test environment with a standard ledger state.
fn create_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000,
        protocol_version: 20,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1000,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 100_000,
    });
    env
}

/// Initialises the contract and returns (client, admin, token_address).
fn setup_contract(env: &Env) -> (BountyEscrowContractClient, Address, Address) {
    let admin = Address::generate(env);
    let token = Address::generate(env); // mock token address
    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(env, &contract_id);
    client.initialize(&admin, &token);
    (client, admin, token)
}

// ============================================================================
// Blacklist Tests
// ============================================================================

/// A blacklisted depositor must not be able to lock funds.
#[test]
fn test_blacklisted_depositor_cannot_lock_funds() {
    let env = create_env();
    let (client, admin, _token) = setup_contract(&env);

    let depositor = Address::generate(&env);
    let bounty_id: u64 = 1;
    let amount: i128 = 1_000;
    let deadline: u64 = env.ledger().timestamp() + 86_400; // +1 day
    let reason = Some(String::from_str(&env, "Sanctioned address"));

    // Admin blacklists the depositor.
    client.set_blacklist(&depositor, &true, &reason);

    // Attempt to lock funds – must be rejected.
    let result = client.try_lock_funds(&bounty_id, &depositor, &amount, &deadline);
    assert_eq!(result, Err(Ok(Error::ParticipantNotAllowed)));
}

/// A blacklisted recipient must not be able to receive a payout.
#[test]
fn test_blacklisted_recipient_cannot_receive_funds() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let bounty_id: u64 = 2;
    let amount: i128 = 500;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    // Lock funds with a clean depositor.
    client
        .lock_funds(&bounty_id, &depositor, &amount, &deadline)
        .unwrap();

    // Now blacklist the recipient before the release.
    client.set_blacklist(&recipient, &true, &None);

    // Release should be rejected.
    let result = client.try_release_funds(&bounty_id, &recipient);
    assert_eq!(result, Err(Ok(Error::ParticipantNotAllowed)));
}

/// Removing an address from the blacklist should restore access.
#[test]
fn test_unblacklisted_address_can_lock_funds() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let depositor = Address::generate(&env);
    let bounty_id: u64 = 3;
    let amount: i128 = 250;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    // Blacklist then immediately unblacklist.
    client.set_blacklist(&depositor, &true, &None);
    client.set_blacklist(&depositor, &false, &None);

    // Lock should now succeed.
    let result = client.try_lock_funds(&bounty_id, &depositor, &amount, &deadline);
    assert!(result.is_ok());
}

/// Blacklisting one address must not affect other addresses.
#[test]
fn test_blacklist_does_not_affect_other_addresses() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let bad_actor = Address::generate(&env);
    let good_user = Address::generate(&env);
    let bounty_id: u64 = 4;
    let amount: i128 = 750;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    client.set_blacklist(&bad_actor, &true, &None);

    // The good user should still be allowed.
    let result = client.try_lock_funds(&bounty_id, &good_user, &amount, &deadline);
    assert!(result.is_ok());
}

/// A blacklisted address cannot act as both depositor AND recipient.
#[test]
fn test_blacklisted_address_blocked_as_both_roles() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let address = Address::generate(&env);
    let bounty_id: u64 = 5;
    let amount: i128 = 100;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    client.set_blacklist(&address, &true, &None);

    // Blocked as depositor.
    let lock_result = client.try_lock_funds(&bounty_id, &address, &amount, &deadline);
    assert_eq!(lock_result, Err(Ok(Error::ParticipantNotAllowed)));

    // Blocked as recipient (lock with a clean depositor first).
    let depositor = Address::generate(&env);
    client
        .lock_funds(&bounty_id, &depositor, &amount, &deadline)
        .unwrap();

    let release_result = client.try_release_funds(&bounty_id, &address);
    assert_eq!(release_result, Err(Ok(Error::ParticipantNotAllowed)));
}

// ============================================================================
// Whitelist-Only Mode Tests
// ============================================================================

/// When whitelist mode is off, any non-blacklisted address can lock funds.
#[test]
fn test_without_whitelist_mode_any_address_can_participate() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let user = Address::generate(&env);
    let bounty_id: u64 = 10;
    let amount: i128 = 300;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    // Whitelist mode is off by default.
    let result = client.try_lock_funds(&bounty_id, &user, &amount, &deadline);
    assert!(result.is_ok());
}

/// When whitelist mode is on, a non-whitelisted depositor is rejected.
#[test]
fn test_whitelist_mode_blocks_non_whitelisted_depositor() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let user = Address::generate(&env);
    let bounty_id: u64 = 11;
    let amount: i128 = 400;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    client.set_whitelist_mode(&true);

    let result = client.try_lock_funds(&bounty_id, &user, &amount, &deadline);
    assert_eq!(result, Err(Ok(Error::ParticipantNotAllowed)));
}

/// When whitelist mode is on, a whitelisted depositor is allowed.
#[test]
fn test_whitelist_mode_allows_whitelisted_depositor() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let user = Address::generate(&env);
    let bounty_id: u64 = 12;
    let amount: i128 = 600;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    client.set_whitelist_mode(&true);
    client.set_whitelist(&user, &true);

    let result = client.try_lock_funds(&bounty_id, &user, &amount, &deadline);
    assert!(result.is_ok());
}

/// When whitelist mode is on, a whitelisted recipient can receive funds.
#[test]
fn test_whitelist_mode_allows_whitelisted_recipient() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env);
    let bounty_id: u64 = 13;
    let amount: i128 = 800;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    // Whitelist both parties before enabling mode so the lock succeeds.
    client.set_whitelist(&depositor, &true);
    client.set_whitelist(&recipient, &true);
    client.set_whitelist_mode(&true);

    client
        .lock_funds(&bounty_id, &depositor, &amount, &deadline)
        .unwrap();

    let result = client.try_release_funds(&bounty_id, &recipient);
    assert!(result.is_ok());
}

/// When whitelist mode is on, a non-whitelisted recipient is blocked.
#[test]
fn test_whitelist_mode_blocks_non_whitelisted_recipient() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let depositor = Address::generate(&env);
    let recipient = Address::generate(&env); // NOT whitelisted
    let bounty_id: u64 = 14;
    let amount: i128 = 900;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    // Whitelist only the depositor so the lock can proceed.
    client.set_whitelist(&depositor, &true);
    client.set_whitelist_mode(&true);

    client
        .lock_funds(&bounty_id, &depositor, &amount, &deadline)
        .unwrap();

    let result = client.try_release_funds(&bounty_id, &recipient);
    assert_eq!(result, Err(Ok(Error::ParticipantNotAllowed)));
}

/// Disabling whitelist mode after it was active should allow all clean addresses again.
#[test]
fn test_disabling_whitelist_mode_restores_access() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let user = Address::generate(&env);
    let bounty_id: u64 = 15;
    let amount: i128 = 350;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    client.set_whitelist_mode(&true);

    // Rejected while mode is on.
    let blocked = client.try_lock_funds(&bounty_id, &user, &amount, &deadline);
    assert_eq!(blocked, Err(Ok(Error::ParticipantNotAllowed)));

    // Allowed after mode is turned off.
    client.set_whitelist_mode(&false);
    let allowed = client.try_lock_funds(&bounty_id, &user, &amount, &deadline);
    assert!(allowed.is_ok());
}

/// Removing an address from the whitelist while whitelist mode is on should block it.
#[test]
fn test_removing_from_whitelist_blocks_address_in_whitelist_mode() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let user = Address::generate(&env);
    let bounty_id_a: u64 = 16;
    let bounty_id_b: u64 = 17;
    let amount: i128 = 200;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    client.set_whitelist(&user, &true);
    client.set_whitelist_mode(&true);

    // First lock succeeds.
    client
        .lock_funds(&bounty_id_a, &user, &amount, &deadline)
        .unwrap();

    // Remove from whitelist.
    client.set_whitelist(&user, &false);

    // Second lock should now fail.
    let result = client.try_lock_funds(&bounty_id_b, &user, &amount, &deadline);
    assert_eq!(result, Err(Ok(Error::ParticipantNotAllowed)));
}

// ============================================================================
// Combined Blacklist + Whitelist Mode Tests
// ============================================================================

/// A blacklisted address should be rejected even if it is also whitelisted.
#[test]
fn test_blacklisted_address_rejected_even_if_whitelisted() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let user = Address::generate(&env);
    let bounty_id: u64 = 20;
    let amount: i128 = 500;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    // Add to whitelist AND blacklist.
    client.set_whitelist(&user, &true);
    client.set_blacklist(&user, &true, &Some(String::from_str(&env, "Fraud")));
    client.set_whitelist_mode(&true);

    let result = client.try_lock_funds(&bounty_id, &user, &amount, &deadline);
    // Blacklist always takes precedence.
    assert_eq!(result, Err(Ok(Error::ParticipantNotAllowed)));
}

/// Blacklist enforcement is independent of whether whitelist mode is on or off.
#[test]
fn test_blacklist_enforced_regardless_of_whitelist_mode() {
    let env = create_env();
    let (client, _admin, _token) = setup_contract(&env);

    let user = Address::generate(&env);
    let bounty_id: u64 = 21;
    let amount: i128 = 150;
    let deadline: u64 = env.ledger().timestamp() + 86_400;

    client.set_blacklist(&user, &true, &None);

    // Whitelist mode OFF – still blocked.
    let result_off = client.try_lock_funds(&bounty_id, &user, &amount, &deadline);
    assert_eq!(result_off, Err(Ok(Error::ParticipantNotAllowed)));

    // Whitelist mode ON – still blocked.
    client.set_whitelist_mode(&true);
    let result_on = client.try_lock_funds(&bounty_id, &user, &amount, &deadline);
    assert_eq!(result_on, Err(Ok(Error::ParticipantNotAllowed)));
}

// ============================================================================
// Admin-Only Access Tests
// ============================================================================

/// Only the admin can modify the blacklist; other callers must be rejected.
/// NOTE: With `mock_all_auths` this test verifies the admin auth is *required*.
/// In a real environment remove `mock_all_auths` and sign with a different key.
#[test]
fn test_only_admin_can_set_blacklist() {
    let env = Env::default(); // No mock_all_auths
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000,
        protocol_version: 20,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1000,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 100_000,
    });
    env.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let token = Address::generate(&env);

    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(&env, &contract_id);
    client.initialize(&admin, &token);

    let target = Address::generate(&env);

    // Admin call must succeed (auth is satisfied by mock).
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "set_blacklist",
            args: (&target, &true, &None::<String>).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    client.set_blacklist(&target, &true, &None);

    // Non-admin call must panic / fail auth.
    env.mock_auths(&[MockAuth {
        address: &non_admin,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "set_blacklist",
            args: (&target, &false, &None::<String>).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    let result = client.try_set_blacklist(&target, &false, &None);
    assert!(result.is_err());
}

/// Only the admin can enable whitelist mode.
#[test]
fn test_only_admin_can_set_whitelist_mode() {
    let env = Env::default();
    env.ledger().set(LedgerInfo {
        timestamp: 1_000_000,
        protocol_version: 20,
        sequence_number: 100,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 1000,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 100_000,
    });
    env.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let token = Address::generate(&env);

    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(&env, &contract_id);
    client.initialize(&admin, &token);

    // Non-admin attempting to toggle whitelist mode must fail.
    env.mock_auths(&[MockAuth {
        address: &non_admin,
        invoke: &MockAuthInvoke {
            contract: &contract_id,
            fn_name: "set_whitelist_mode",
            args: (&true,).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    let result = client.try_set_whitelist_mode(&true);
    assert!(result.is_err());
}