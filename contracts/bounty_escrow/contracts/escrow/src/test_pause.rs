#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract_address = e
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    (
        token::Client::new(e, &contract_address),
        token::StellarAssetClient::new(e, &contract_address),
    )
}

fn create_escrow_contract<'a>(e: &Env) -> (BountyEscrowContractClient<'a>, Address) {
    let contract_id = e.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(e, &contract_id);
    (client, contract_id)
}

#[test]
fn test_granular_pause_lock() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_client, token_admin_client) = create_token_contract(&env, &token_admin);
    let (escrow_client, _escrow_address) = create_escrow_contract(&env);

    // Initialize using the correct `init` function
    escrow_client
        .init(&admin, &token_client.address)
        .unwrap();

    // Check default state (all unpaused)
    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.lock_paused, false);
    assert_eq!(flags.release_paused, false);
    assert_eq!(flags.refund_paused, false);

    // Setup funds
    token_admin_client.mint(&depositor, &1000);

    // Verify lock works when unpaused
    let bounty_id_1: u64 = 1;
    let deadline = env.ledger().timestamp() + 1000;
    escrow_client
        .lock_funds(&depositor, &bounty_id_1, &100, &deadline)
        .unwrap();

    // Pause lock
    escrow_client
        .set_paused(&Some(true), &None, &None)
        .unwrap();

    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.lock_paused, true);

    // Try to lock while paused — should return FundsPaused error
    let bounty_id_2: u64 = 2;
    let res =
        escrow_client.try_lock_funds(&depositor, &bounty_id_2, &100, &deadline);
    assert_eq!(res, Err(Ok(Error::FundsPaused)));

    // Unpause lock
    escrow_client
        .set_paused(&Some(false), &None, &None)
        .unwrap();

    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.lock_paused, false);

    // Lock should work again
    escrow_client
        .lock_funds(&depositor, &bounty_id_2, &100, &deadline)
        .unwrap();
}

#[test]
fn test_granular_pause_release() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contributor = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_client, token_admin_client) = create_token_contract(&env, &token_admin);
    let (escrow_client, _) = create_escrow_contract(&env);

    escrow_client
        .init(&admin, &token_client.address)
        .unwrap();
    token_admin_client.mint(&depositor, &1000);

    let bounty_id: u64 = 1;
    let deadline = env.ledger().timestamp() + 1000;
    escrow_client
        .lock_funds(&depositor, &bounty_id, &100, &deadline)
        .unwrap();

    // Pause release
    escrow_client
        .set_paused(&None, &Some(true), &None)
        .unwrap();

    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.release_paused, true);

    // Try to release while paused — should return FundsPaused
    let res = escrow_client.try_release_funds(&bounty_id, &contributor);
    assert_eq!(res, Err(Ok(Error::FundsPaused)));

    // Unpause release
    escrow_client
        .set_paused(&None, &Some(false), &None)
        .unwrap();

    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.release_paused, false);

    // Release should now succeed
    escrow_client
        .release_funds(&bounty_id, &contributor)
        .unwrap();
}

#[test]
fn test_granular_pause_refund() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let (token_client, token_admin_client) = create_token_contract(&env, &token_admin);
    let (escrow_client, _) = create_escrow_contract(&env);

    escrow_client
        .init(&admin, &token_client.address)
        .unwrap();
    token_admin_client.mint(&depositor, &1000);

    let bounty_id: u64 = 1;
    let deadline = env.ledger().timestamp(); // deadline = now, so it's already passed

    escrow_client
        .lock_funds(&depositor, &bounty_id, &100, &deadline)
        .unwrap();

    // Advance time past the deadline
    env.ledger().set_timestamp(deadline + 1);

    // Pause refund
    escrow_client
        .set_paused(&None, &None, &Some(true))
        .unwrap();

    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.refund_paused, true);

    // Try to refund while paused — should return FundsPaused
    let res = escrow_client.try_refund(
        &bounty_id,
        &None::<i128>,
        &None::<Address>,
        &RefundMode::Full,
    );
    assert_eq!(res, Err(Ok(Error::FundsPaused)));

    // Unpause refund
    escrow_client
        .set_paused(&None, &None, &Some(false))
        .unwrap();

    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.refund_paused, false);

    // Refund should now succeed
    escrow_client
        .refund(
            &bounty_id,
            &None::<i128>,
            &None::<Address>,
            &RefundMode::Full,
        )
        .unwrap();
}

#[test]
fn test_mixed_pause_states() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (token_client, _) = create_token_contract(&env, &admin);
    let (escrow_client, _) = create_escrow_contract(&env);

    escrow_client
        .init(&admin, &token_client.address)
        .unwrap();

    // Pause lock and release, but leave refund unpaused
    escrow_client
        .set_paused(&Some(true), &Some(true), &Some(false))
        .unwrap();

    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.lock_paused, true);
    assert_eq!(flags.release_paused, true);
    assert_eq!(flags.refund_paused, false);

    // Update only release back to unpaused — lock should remain paused
    escrow_client
        .set_paused(&None, &Some(false), &None)
        .unwrap();

    let flags = escrow_client.get_pause_flags();
    assert_eq!(flags.lock_paused, true);
    assert_eq!(flags.release_paused, false);
    assert_eq!(flags.refund_paused, false);
}
