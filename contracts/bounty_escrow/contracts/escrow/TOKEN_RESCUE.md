# Safe Token Rescue Mechanism

## Overview

The token rescue mechanism allows the contract admin to recover tokens that were accidentally sent directly to the contract address and are not associated with any escrow. This feature ensures that user funds in escrows are never at risk while providing a way to recover mistakenly transferred tokens.

## Problem Statement

Users may accidentally transfer tokens directly to the contract address instead of using the proper `lock_funds` function. These tokens would be stuck in the contract forever without a rescue mechanism. However, any rescue mechanism must be carefully designed to ensure it cannot be used to steal funds that are legitimately locked in escrows.

## Solution

The rescue mechanism computes an "untracked balance" as:

```
untracked_balance = contract_token_balance - sum(escrow.remaining_amount for all active escrows)
```

Only tokens in excess of tracked escrow balances can be rescued. This ensures that escrow-managed funds are never touched.

## Key Functions

### `set_treasury_address(treasury: Address)`

Sets the treasury address where rescued tokens will be sent.

**Authorization:** Admin only

**Parameters:**
- `treasury`: Address to receive rescued tokens

**Errors:**
- `NotInitialized`: Contract not initialized
- `Unauthorized`: Caller is not admin

**Events:**
- `TreasuryUpdated`: Emitted when treasury address is set

### `get_treasury_address() -> Option<Address>`

Returns the current treasury address if configured.

**Returns:**
- `Some(Address)`: Treasury address if set
- `None`: Treasury address not configured

### `rescue_untracked_tokens(amount: i128)`

Rescues tokens that are not associated with any escrow.

**Authorization:** Admin only

**Parameters:**
- `amount`: Amount of untracked tokens to rescue (must be ≤ untracked balance)

**Errors:**
- `NotInitialized`: Contract not initialized
- `Unauthorized`: Caller is not admin
- `FeeRecipientNotSet`: Treasury address not configured
- `InvalidAmount`: Amount is zero, negative, or exceeds untracked balance
- `NoUntrackedBalance`: No untracked tokens available to rescue

**Events:**
- `TokensRescued`: Emitted with full audit trail including:
  - Admin address
  - Treasury address
  - Amount rescued
  - Contract balance before rescue
  - Tracked balance
  - Untracked balance
  - Timestamp

**Reentrancy Protection:** Protected by the shared reentrancy guard

### `get_untracked_balance() -> (i128, i128, i128)`

View function to check untracked token balance.

**Returns:**
Tuple of `(contract_balance, tracked_balance, untracked_balance)`

## Safety Guarantees

1. **Escrow Protection**: Only untracked tokens (not part of any escrow) can be rescued
2. **Balance Tracking**: Escrow balances are calculated by summing `remaining_amount` from all active escrows (Locked or PartiallyRefunded status)
3. **Released/Refunded Escrows**: Escrows with Released or Refunded status have `remaining_amount = 0` and don't contribute to tracked balance
4. **Authorization**: Requires admin authorization
5. **Treasury Configuration**: Requires treasury address to be configured before rescue
6. **Audit Trail**: Emits comprehensive event for full transparency
7. **Reentrancy Protection**: Uses the same reentrancy guard as other critical functions

## Usage Example

```rust
// 1. Set treasury address (one-time setup)
contract.set_treasury_address(&treasury_address);

// 2. Check if there are untracked tokens
let (contract_balance, tracked_balance, untracked_balance) = 
    contract.get_untracked_balance();

if untracked_balance > 0 {
    // 3. Rescue the untracked tokens
    contract.rescue_untracked_tokens(&untracked_balance);
}
```

## Edge Cases Handled

1. **Multiple Escrows**: Correctly sums tracked balance across all active escrows
2. **Partial Releases**: Accounts for `remaining_amount` after partial releases
3. **Partial Refunds**: Includes PartiallyRefunded escrows in tracked balance calculation
4. **Released Escrows**: Ignores Released escrows (remaining_amount = 0)
5. **Refunded Escrows**: Ignores Refunded escrows (remaining_amount = 0)
6. **Zero Balance**: Returns error if no untracked tokens available
7. **Overflow Protection**: Uses saturating arithmetic to prevent overflow

## Testing

Comprehensive test suite in `test_token_rescue.rs` covers:

- Basic rescue functionality
- Partial rescue
- Protection of escrow funds
- Multiple escrows
- Partial releases and refunds
- Released and refunded escrows
- Authorization checks
- Treasury configuration
- Error conditions
- View function accuracy

All 16 tests pass successfully.

## Security Considerations

### What Can Be Rescued
- Tokens sent directly to the contract address by mistake
- Tokens that exceed the sum of all active escrow balances

### What Cannot Be Rescued
- Tokens locked in active escrows (Locked status)
- Tokens in partially refunded escrows (PartiallyRefunded status)
- Any amount that would reduce the contract balance below tracked escrow balances

### Attack Vectors Mitigated
1. **Admin Theft**: Cannot rescue escrow-managed funds
2. **Calculation Errors**: Uses simple, auditable arithmetic
3. **Race Conditions**: Protected by reentrancy guard
4. **Unauthorized Access**: Requires admin authorization
5. **Missing Treasury**: Requires treasury configuration before rescue

## Events

### TreasuryUpdated
```rust
pub struct TreasuryUpdated {
    pub treasury: Address,
    pub timestamp: u64,
}
```

### TokensRescued
```rust
pub struct TokensRescued {
    pub admin: Address,
    pub treasury: Address,
    pub amount: i128,
    pub contract_balance_before: i128,
    pub tracked_balance: i128,
    pub untracked_balance: i128,
    pub timestamp: u64,
}
```

The `TokensRescued` event provides complete transparency by including:
- Who initiated the rescue (admin)
- Where tokens were sent (treasury)
- How much was rescued (amount)
- Contract state before rescue (contract_balance_before)
- How much was tracked in escrows (tracked_balance)
- How much was available to rescue (untracked_balance)
- When the rescue occurred (timestamp)

This allows off-chain systems to verify that the rescue was legitimate and did not touch escrow funds.

## Integration Notes

1. **Treasury Setup**: Call `set_treasury_address` during contract initialization or before first rescue
2. **Monitoring**: Use `get_untracked_balance` to monitor for accidentally sent tokens
3. **Automation**: Can be automated to periodically check and rescue untracked tokens
4. **Event Indexing**: Index `TokensRescued` events for audit trail and reporting

## Future Enhancements

Potential improvements for future versions:
1. Multi-signature requirement for rescue operations
2. Time-lock delay before rescue can be executed
3. Automatic rescue to treasury when untracked balance exceeds threshold
4. Whitelist of addresses that can receive rescued tokens
