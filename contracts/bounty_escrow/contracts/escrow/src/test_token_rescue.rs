//! Tests for safe token rescue mechanism
//!
//! This module tests the ability to rescue tokens that were accidentally sent
//! directly to the contract address (not associated with any escrow).

use crate::{BountyEscrowContract, BountyEscrowContractClient, Error, EscrowStatus};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token, Address, Env,
};

struct TestSetup {
    env: Env,
    contract_id: Address,
    client: BountyEscrowContractClient<'static>,
    token: token::TokenClient<'static>,
    token_id: Address,
    admin: Address,
    depositor: Address,
    contributor: Address,
    treasury: Address,
}

impl TestSetup {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let contributor = Address::generate(&env);
        let treasury = Address::generate(&env);

        let token_id = env.register_stellar_asset_contract(admin.clone());
        let token = token::TokenClient::new(&env, &token_id);
        let token_admin = token::StellarAssetClient::new(&env, &token_id);

        let contract_id = env.register_contract(None, BountyEscrowContract);
        let client = BountyEscrowContractClient::new(&env, &contract_id);

        client.init(&admin, &token_id);

        Self {
            env,
            contract_id,
            client,
            token,
            token_id,
            admin,
            depositor,
            contributor,
            treasury,
        }
    }

    fn mint_to(&self, recipient: &Address, amount: i128) {
        let token_admin = token::StellarAssetClient::new(&self.env, &self.token_id);
        token_admin.mint(recipient, &amount);
    }
}

#[test]
fn test_set_treasury_address() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Verify treasury address is set
    let stored_treasury = setup.client.get_treasury_address();
    assert_eq!(stored_treasury, Some(setup.treasury.clone()));
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_set_treasury_requires_admin() {
    let setup = TestSetup::new();
    let non_admin = Address::generate(&setup.env);

    // Disable mock_all_auths to test actual authorization
    setup.env.set_auths(&[]);

    // Try to set treasury as non-admin (should fail)
    setup.client.set_treasury_address(&non_admin);
}

#[test]
fn test_rescue_untracked_tokens_basic() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Send tokens directly to contract (simulating accidental transfer)
    let accidental_amount = 5000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Check untracked balance
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    assert_eq!(contract_balance, accidental_amount);
    assert_eq!(tracked_balance, 0);
    assert_eq!(untracked_balance, accidental_amount);

    // Rescue the untracked tokens
    setup.client.rescue_untracked_tokens(&accidental_amount);

    // Verify tokens were transferred to treasury
    assert_eq!(setup.token.balance(&setup.treasury), accidental_amount);
    assert_eq!(setup.token.balance(&setup.contract_id), 0);
}

#[test]
fn test_rescue_partial_untracked_tokens() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Send tokens directly to contract
    let accidental_amount = 10000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Rescue only part of the untracked tokens
    let rescue_amount = 6000i128;
    setup.client.rescue_untracked_tokens(&rescue_amount);

    // Verify partial rescue
    assert_eq!(setup.token.balance(&setup.treasury), rescue_amount);
    assert_eq!(
        setup.token.balance(&setup.contract_id),
        accidental_amount - rescue_amount
    );
}

#[test]
fn test_rescue_does_not_touch_escrow_funds() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Create an escrow with tracked funds
    let bounty_id = 1u64;
    let escrow_amount = 10000i128;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup.mint_to(&setup.depositor, escrow_amount);
    setup
        .client
        .lock_funds(&setup.depositor, &bounty_id, &escrow_amount, &deadline);

    // Send additional tokens directly to contract (untracked)
    let accidental_amount = 5000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Check balances
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    assert_eq!(contract_balance, escrow_amount + accidental_amount);
    assert_eq!(tracked_balance, escrow_amount);
    assert_eq!(untracked_balance, accidental_amount);

    // Rescue only the untracked tokens
    setup.client.rescue_untracked_tokens(&accidental_amount);

    // Verify escrow funds are untouched
    let escrow = setup.client.get_escrow_info(&bounty_id);
    assert_eq!(escrow.remaining_amount, escrow_amount);
    assert_eq!(escrow.status, EscrowStatus::Locked);

    // Verify contract still has escrow funds
    assert_eq!(setup.token.balance(&setup.contract_id), escrow_amount);

    // Verify treasury received untracked tokens
    assert_eq!(setup.token.balance(&setup.treasury), accidental_amount);
}

#[test]
fn test_rescue_with_multiple_escrows() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Create multiple escrows
    let escrow1_amount = 3000i128;
    let escrow2_amount = 7000i128;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup.mint_to(&setup.depositor, escrow1_amount + escrow2_amount);
    setup
        .client
        .lock_funds(&setup.depositor, &1u64, &escrow1_amount, &deadline);
    setup
        .client
        .lock_funds(&setup.depositor, &2u64, &escrow2_amount, &deadline);

    // Send untracked tokens
    let accidental_amount = 2000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Check balances
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    assert_eq!(
        contract_balance,
        escrow1_amount + escrow2_amount + accidental_amount
    );
    assert_eq!(tracked_balance, escrow1_amount + escrow2_amount);
    assert_eq!(untracked_balance, accidental_amount);

    // Rescue untracked tokens
    setup.client.rescue_untracked_tokens(&accidental_amount);

    // Verify all escrows are intact
    let escrow1 = setup.client.get_escrow_info(&1u64);
    let escrow2 = setup.client.get_escrow_info(&2u64);
    assert_eq!(escrow1.remaining_amount, escrow1_amount);
    assert_eq!(escrow2.remaining_amount, escrow2_amount);

    // Verify treasury received untracked tokens
    assert_eq!(setup.token.balance(&setup.treasury), accidental_amount);
}

#[test]
fn test_rescue_after_partial_release() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Create escrow
    let bounty_id = 1u64;
    let escrow_amount = 10000i128;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup.mint_to(&setup.depositor, escrow_amount);
    setup
        .client
        .lock_funds(&setup.depositor, &bounty_id, &escrow_amount, &deadline);

    // Partially release funds
    let release_amount = 6000i128;
    setup
        .client
        .partial_release(&bounty_id, &setup.contributor, &release_amount);

    // Send untracked tokens
    let accidental_amount = 3000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Check balances (tracked should be remaining_amount after partial release)
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    let expected_tracked = escrow_amount - release_amount;
    assert_eq!(contract_balance, expected_tracked + accidental_amount);
    assert_eq!(tracked_balance, expected_tracked);
    assert_eq!(untracked_balance, accidental_amount);

    // Rescue untracked tokens
    setup.client.rescue_untracked_tokens(&accidental_amount);

    // Verify escrow remaining amount is correct
    let escrow = setup.client.get_escrow_info(&bounty_id);
    assert_eq!(escrow.remaining_amount, expected_tracked);

    // Verify treasury received untracked tokens
    assert_eq!(setup.token.balance(&setup.treasury), accidental_amount);
}

#[test]
fn test_rescue_fails_without_treasury() {
    let setup = TestSetup::new();

    // Don't set treasury address

    // Send tokens to contract
    let accidental_amount = 5000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Try to rescue without treasury set (should fail)
    let result = setup.client.try_rescue_untracked_tokens(&accidental_amount);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), Error::FeeRecipientNotSet);
}

#[test]
fn test_rescue_fails_with_zero_amount() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Send tokens to contract
    setup.mint_to(&setup.contract_id, 5000i128);

    // Try to rescue zero amount (should fail)
    let result = setup.client.try_rescue_untracked_tokens(&0);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), Error::InvalidAmount);
}

#[test]
fn test_rescue_fails_with_negative_amount() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Send tokens to contract
    setup.mint_to(&setup.contract_id, 5000i128);

    // Try to rescue negative amount (should fail)
    let result = setup.client.try_rescue_untracked_tokens(&-100);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), Error::InvalidAmount);
}

#[test]
fn test_rescue_fails_when_exceeding_untracked_balance() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Send tokens to contract
    let accidental_amount = 5000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Try to rescue more than available (should fail)
    let result = setup.client.try_rescue_untracked_tokens(&(accidental_amount + 1));
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), Error::InvalidAmount);
}

#[test]
fn test_rescue_fails_when_no_untracked_balance() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Create escrow (all funds are tracked)
    let bounty_id = 1u64;
    let escrow_amount = 10000i128;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup.mint_to(&setup.depositor, escrow_amount);
    setup
        .client
        .lock_funds(&setup.depositor, &bounty_id, &escrow_amount, &deadline);

    // Try to rescue when all funds are tracked (should fail)
    let result = setup.client.try_rescue_untracked_tokens(&100);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().unwrap(), Error::NoUntrackedBalance);
}

#[test]
#[should_panic(expected = "HostError: Error(Auth, InvalidAction)")]
fn test_rescue_requires_admin() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Send tokens to contract
    setup.mint_to(&setup.contract_id, 5000i128);

    // Disable mock_all_auths to test actual authorization
    setup.env.set_auths(&[]);

    // Try to rescue as non-admin (should fail)
    setup.client.rescue_untracked_tokens(&1000);
}

#[test]
fn test_get_untracked_balance_view() {
    let setup = TestSetup::new();

    // Initially, all balances should be zero
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    assert_eq!(contract_balance, 0);
    assert_eq!(tracked_balance, 0);
    assert_eq!(untracked_balance, 0);

    // Create escrow
    let escrow_amount = 8000i128;
    let deadline = setup.env.ledger().timestamp() + 1000;
    setup.mint_to(&setup.depositor, escrow_amount);
    setup
        .client
        .lock_funds(&setup.depositor, &1u64, &escrow_amount, &deadline);

    // Check balances after escrow
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    assert_eq!(contract_balance, escrow_amount);
    assert_eq!(tracked_balance, escrow_amount);
    assert_eq!(untracked_balance, 0);

    // Send untracked tokens
    let accidental_amount = 2000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Check balances with untracked tokens
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    assert_eq!(contract_balance, escrow_amount + accidental_amount);
    assert_eq!(tracked_balance, escrow_amount);
    assert_eq!(untracked_balance, accidental_amount);
}

#[test]
fn test_rescue_with_partially_refunded_escrow() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Create escrow
    let bounty_id = 1u64;
    let escrow_amount = 10000i128;
    let deadline = setup.env.ledger().timestamp() + 1000;

    setup.mint_to(&setup.depositor, escrow_amount);
    setup
        .client
        .lock_funds(&setup.depositor, &bounty_id, &escrow_amount, &deadline);

    // Approve and execute partial refund
    let refund_amount = 3000i128;
    setup.client.approve_refund(
        &bounty_id,
        &refund_amount,
        &setup.depositor,
        &crate::RefundMode::Partial,
    );
    setup.client.refund(&bounty_id);

    // Send untracked tokens
    let accidental_amount = 1000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Check balances
    let expected_tracked = escrow_amount - refund_amount;
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    assert_eq!(contract_balance, expected_tracked + accidental_amount);
    assert_eq!(tracked_balance, expected_tracked);
    assert_eq!(untracked_balance, accidental_amount);

    // Rescue untracked tokens
    setup.client.rescue_untracked_tokens(&accidental_amount);

    // Verify escrow is still partially refunded with correct remaining amount
    let escrow = setup.client.get_escrow_info(&bounty_id);
    assert_eq!(escrow.status, EscrowStatus::PartiallyRefunded);
    assert_eq!(escrow.remaining_amount, expected_tracked);

    // Verify treasury received untracked tokens
    assert_eq!(setup.token.balance(&setup.treasury), accidental_amount);
}

#[test]
fn test_rescue_ignores_released_and_refunded_escrows() {
    let setup = TestSetup::new();

    // Set treasury address
    setup.client.set_treasury_address(&setup.treasury);

    // Create and release one escrow
    let bounty1_amount = 5000i128;
    let deadline = setup.env.ledger().timestamp() + 1000;
    setup.mint_to(&setup.depositor, bounty1_amount);
    setup
        .client
        .lock_funds(&setup.depositor, &1u64, &bounty1_amount, &deadline);
    setup.client.release_funds(&1u64, &setup.contributor);

    // Create and refund another escrow
    let bounty2_amount = 3000i128;
    setup.mint_to(&setup.depositor, bounty2_amount);
    setup
        .client
        .lock_funds(&setup.depositor, &2u64, &bounty2_amount, &deadline);
    setup.env.ledger().set_timestamp(deadline + 1);
    setup.client.refund(&2u64);

    // Create one active escrow
    let bounty3_amount = 4000i128;
    setup.mint_to(&setup.depositor, bounty3_amount);
    let new_deadline = deadline + 2000;
    setup
        .client
        .lock_funds(&setup.depositor, &3u64, &bounty3_amount, &new_deadline);

    // Send untracked tokens
    let accidental_amount = 2000i128;
    setup.mint_to(&setup.contract_id, accidental_amount);

    // Check balances - only bounty3 should be tracked
    let (contract_balance, tracked_balance, untracked_balance) =
        setup.client.get_untracked_balance();
    assert_eq!(contract_balance, bounty3_amount + accidental_amount);
    assert_eq!(tracked_balance, bounty3_amount); // Only active escrow
    assert_eq!(untracked_balance, accidental_amount);

    // Rescue untracked tokens
    setup.client.rescue_untracked_tokens(&accidental_amount);

    // Verify treasury received untracked tokens
    assert_eq!(setup.token.balance(&setup.treasury), accidental_amount);
}
