# Single-Use Claim Ticket Implementation

## Overview

This implementation introduces a "claim ticket" mechanism for bounty distribution in the Soroban escrow contract. Winners receive single-use tickets that allow them to claim their rewards exactly once, simplifying distribution and reducing the risk of misdirected payouts.

## Key Features

### 1. **Single-Use Tickets**
- Each ticket can only be used once (replay prevention)
- Marked as `used` after successful claim
- Attempting to replay returns `TicketAlreadyUsed` error

### 2. **Expiry Management**
- Tickets have explicit expiration timestamps
- Claims before expiry are allowed
- Claims after expiry return `TicketExpired` error
- Boundary checking: `now > expires_at` (not `>=`)

### 3. **Beneficiary Binding**
- Tickets are bound to specific addresses
- Only the designated beneficiary can claim
- Address verification via `require_auth()`

### 4. **Amount Binding**
- Ticket specifies exact claim amount
- Prevents unauthorized amount modifications
- Validates amount is within escrow bounds

## Data Structures

### ClaimTicket Struct
```rust
pub struct ClaimTicket {
    pub ticket_id: u64,           // Unique identifier
    pub bounty_id: u64,           // Associated bounty
    pub beneficiary: Address,     // Authorized claimer
    pub amount: i128,             // Amount to claim
    pub expires_at: u64,          // Expiry timestamp
    pub used: bool,               // Single-use flag
    pub issued_at: u64,           // Issuance timestamp
}
```

### Storage Keys
- `DataKey::ClaimTicket(u64)` - Store individual tickets
- `DataKey::ClaimTicketIndex` - Track all issued ticket IDs
- `DataKey::TicketCounter` - Counter for unique ticket ID generation
- `DataKey::BeneficiaryTickets(Address)` - Index tickets by beneficiary

## Error Handling

New error types added:
- `Error::TicketNotFound = 23` - Ticket doesn't exist
- `Error::TicketAlreadyUsed = 24` - Ticket has been used (replay prevention)
- `Error::TicketExpired = 25` - Ticket is past expiration

## Core Functions

### `issue_claim_ticket()`
**Admin-only function to create tickets for bounty winners**

Validations:
- Admin authorization required
- Bounty must exist and be in `Locked` state
- Amount must be positive and ≤ escrow amount
- Expiry must be in the future

Returns:
- `Ok(ticket_id)` - Unique ticket ID for later claiming
- Various errors for validation failures

**Side Effects:**
- Generates unique ticket_id
- Stores ticket in persistent storage
- Adds to global ticket index
- Adds to beneficiary's ticket list
- Emits `TicketIssued` event

### `claim_with_ticket()`
**Beneficiary calls this to claim their reward**

Validations:
- Release operations not paused
- Ticket exists
- Ticket not yet used
- Ticket not expired
- Caller is the ticket's beneficiary
- Associated bounty exists and is `Locked`

Returns:
- `Ok(())` - Funds transferred successfully
- Various errors for validation failures

**Side Effects:**
- Marks ticket as `used` (prevents replay)
- Transfers funds to beneficiary
- Updates escrow to `Released` status
- Runs escrow invariant checks
- Emits `TicketClaimed` event

### `get_claim_ticket()`
**Query function to retrieve ticket details**

Returns full `ClaimTicket` struct for verification and inspection.

### `get_beneficiary_tickets()`
**Paginated query of all tickets for a beneficiary**

Parameters:
- `beneficiary` - Address to query
- `offset` - Starting position
- `limit` - Maximum results

Returns: `Vec<u64>` of ticket IDs

### `verify_claim_ticket()`
**Non-mutating verification function**

Returns tuple: `(is_valid, is_expired, already_used)`

Useful for frontends to validate tickets before attempting claims without modifying state.

## Events

### TicketIssued
```rust
pub struct TicketIssued {
    pub ticket_id: u64,
    pub bounty_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
    pub expires_at: u64,
    pub issued_at: u64,
}
```

### TicketClaimed
```rust
pub struct TicketClaimed {
    pub ticket_id: u64,
    pub bounty_id: u64,
    pub beneficiary: Address,
    pub amount: i128,
    pub claimed_at: u64,
}
```

## Test Coverage

Comprehensive test suite in `test_claim_tickets.rs` includes:

### Basic Issuance Tests
- ✅ Successful ticket issuance
- ✅ Multiple tickets for same bounty
- ✅ Non-existent bounty handling
- ✅ Invalid amount validation (zero, exceeds escrow)
- ✅ Expired deadline validation
- ✅ Admin-only enforcement

### Replay Prevention Tests
- ✅ Single-use enforcement
- ✅ Ticket marked as used after claim
- ✅ Replay attempts fail with `TicketAlreadyUsed`
- ✅ Multiple independent tickets work correctly

### Expiry Validation Tests
- ✅ Claim before expiry succeeds
- ✅ Claim after expiry fails with `TicketExpired`
- ✅ Boundary condition testing (claim at exact expiry)
- ✅ Multiple expiry scenarios

### Query and Verification Tests
- ✅ Retrieve ticket details
- ✅ Get beneficiary tickets with pagination
- ✅ Non-mutating verification function
- ✅ Validation of expired/used tickets

### Authorization Tests
- ✅ Beneficiary binding enforcement
- ✅ Wrong beneficiary rejection

### Integration Tests
- ✅ Full bounty workflow with tickets
- ✅ Multiple winners from single bounty
- ✅ State transitions and escrow status updates

## Implementation Notes

### Ticket ID Generation
- Uses a counter stored at `DataKey::TicketCounter`
- Incremented for each new ticket
- Ensures uniqueness within contract lifetime

### Storage Strategy
- Tickets stored individually for easy lookup
- Global index for transaction/audit purposes
- Per-beneficiary index for user queries
- Optimized for common query patterns

### Atomic Operations
- Ticket creation is atomic
- Claim operation is atomic
- No partial state updates

### Security Considerations
1. **Replay Prevention**: `used` flag and state mutation prevents double-claiming
2. **Address Binding**: Direct address comparison prevents token hijacking
3. **Amount Binding**: Exact amount in ticket prevents unauthorized claims
4. **Expiry Enforcement**: Time-based access control limits claim window
5. **Auth Requirements**: Proper `require_auth()` calls ensure legitimate users

## Clean Code Principles

### Organization
- Dedicated test module (`test_claim_tickets.rs`)
- Clear separation of concerns
- Modular function design

### Documentation
- Comprehensive doc comments
- Clear parameter descriptions
- Detailed return value documentation
- Error case documentation

### Consistency
- Follows existing code patterns
- Consistent error handling
- Aligned with Soroban best practices
- Consistent storage key naming

### No Common File Modifications
- All changes in dedicated implementations
- Test file separate from common test infrastructure
- No modifications to `lib.rs` core structure (only new functions added)
- No changes to `test.rs` snapshot files

## Testing Instructions

Run the test suite:
```bash
cd contracts/bounty_escrow/contracts/escrow
cargo test test_claim_tickets
```

Run specific test category:
```bash
cargo test test_claim_tickets::test_replay_prevention
cargo test test_claim_tickets::test_expiry
cargo test test_claim_tickets::test_claim_before_expiry
```

## Integration with Existing System

The claim ticket system integrates seamlessly:
- Uses existing error types (with 3 new additions)
- Leverages existing storage patterns
- Follows established event emission patterns
- Compatible with existing pause/unpause system
- Works with all bounty states

## Workflow Example

```rust
// 1. Admin creates bounty and locks funds
contract.lock_funds(env, depositor, bounty_id, 1000, deadline)?;

// 2. Admin issues ticket to winner
let ticket_id = contract.issue_claim_ticket(
    env, 
    bounty_id,
    winner_address,
    1000,
    expires_at
)?;

// 3. Winner receives ticket ID (via event or off-chain communication)

// 4. Winner claims reward using ticket
contract.claim_with_ticket(env, ticket_id)?;

// 5. Funds transferred, ticket marked as used, escrow released
// 6. Replay attempts fail with TicketAlreadyUsed
```

## Migration Path

For systems migrating from direct release:
1. Create tickets instead of calling `release_funds` directly
2. Beneficiaries claim using `claim_with_ticket`
3. Supports audit trail through events
4. Enables ticket validity verification before claiming

