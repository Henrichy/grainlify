#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};


fn setup_with_admin(env: &Env) -> (ProgramEscrowContract, Address) {
    let contract = ProgramEscrowContract;
    let admin = Address::generate(env);
    contract.initialize_contract(env, admin.clone());
    (contract, admin)
}

fn setup_program_with_admin(env: &Env) -> (ProgramEscrowContract, Address, Address, String) {
    let (contract, admin) = setup_with_admin(env);
    let payout_key = Address::generate(env);
    let token = Address::generate(env);
    let program_id = String::from_str(env, "pause-test-prog");
    contract.initialize_program(env, program_id.clone(), payout_key.clone(), token.clone());
    (contract, admin, payout_key, program_id)
}

// --- get_pause_flags & default state ---

#[test]
fn test_default_pause_flags_are_all_false() {
    let env = Env::default();
    let (contract, _admin) = setup_with_admin(&env);

    let flags = contract.get_pause_flags(&env);
    assert_eq!(flags.lock_paused, false);
    assert_eq!(flags.release_paused, false);
    assert_eq!(flags.refund_paused, false);
}

// --- set_paused: lock ---

#[test]
fn test_set_paused_lock() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract, _admin) = setup_with_admin(&env);

    contract.set_paused(&env, Some(true), None, None);

    let flags = contract.get_pause_flags(&env);
    assert_eq!(flags.lock_paused, true);
    assert_eq!(flags.release_paused, false);
    assert_eq!(flags.refund_paused, false);
}

#[test]
fn test_unset_paused_lock() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract, _admin) = setup_with_admin(&env);

    contract.set_paused(&env, Some(true), None, None);
    contract.set_paused(&env, Some(false), None, None);

    let flags = contract.get_pause_flags(&env);
    assert_eq!(flags.lock_paused, false);
}

// --- set_paused: release ---

#[test]
fn test_set_paused_release() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract, _admin) = setup_with_admin(&env);

    contract.set_paused(&env, None, Some(true), None);

    let flags = contract.get_pause_flags(&env);
    assert_eq!(flags.lock_paused, false);
    assert_eq!(flags.release_paused, true);
    assert_eq!(flags.refund_paused, false);
}

// --- mixed pause states ---

#[test]
fn test_mixed_pause_states() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract, _admin) = setup_with_admin(&env);

    // Pause lock and release, leave refund unpaused
    contract.set_paused(&env, Some(true), Some(true), Some(false));

    let flags = contract.get_pause_flags(&env);
    assert_eq!(flags.lock_paused, true);
    assert_eq!(flags.release_paused, true);
    assert_eq!(flags.refund_paused, false);

    // Only update release back to unpaused; lock should stay paused
    contract.set_paused(&env, None, Some(false), None);

    let flags = contract.get_pause_flags(&env);
    assert_eq!(flags.lock_paused, true);
    assert_eq!(flags.release_paused, false);
    assert_eq!(flags.refund_paused, false);
}

// --- lock_program_funds enforcement ---

#[test]
#[should_panic(expected = "Funds Paused")]
fn test_lock_program_funds_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract, _admin, _payout_key, program_id) = setup_program_with_admin(&env);

    contract.set_paused(&env, Some(true), None, None);
    contract.lock_program_funds(&env, program_id, 1000);
}

// --- single_payout enforcement ---

#[test]
#[should_panic(expected = "Funds Paused")]
fn test_single_payout_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract, _admin, _payout_key, program_id) = setup_program_with_admin(&env);
    let recipient = Address::generate(&env);

    contract.set_paused(&env, None, Some(true), None);
    contract.single_payout(&env, program_id, recipient, 100);
}

// --- batch_payout enforcement ---

#[test]
#[should_panic(expected = "Funds Paused")]
fn test_batch_payout_paused() {
    let env = Env::default();
    env.mock_all_auths();
    let (contract, _admin, _payout_key, program_id) = setup_program_with_admin(&env);
    let recipient = Address::generate(&env);

    let recipients = soroban_sdk::vec![&env, recipient];
    let amounts = soroban_sdk::vec![&env, 100i128];

    contract.set_paused(&env, None, Some(true), None);
    contract.batch_payout(&env, program_id, recipients, amounts);
}

// --- initialize_contract guard ---

#[test]
#[should_panic(expected = "Already initialized")]
fn test_double_initialize_contract() {
    let env = Env::default();
    let contract = ProgramEscrowContract;
    let admin = Address::generate(&env);

    contract.initialize_contract(&env, admin.clone());
    contract.initialize_contract(&env, admin); // should panic
}

// --- set_paused requires initialization ---

#[test]
#[should_panic(expected = "Not initialized")]
fn test_set_paused_before_initialize() {
    let env = Env::default();
    let contract = ProgramEscrowContract;

    contract.set_paused(&env, Some(true), None, None);
}
