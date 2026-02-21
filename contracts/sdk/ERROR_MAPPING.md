# Error Mapping Documentation

This document describes how errors from the Soroban smart contracts are mapped to typed SDK errors.

## Error Flow

```
Contract Error → parseContractError() → Typed ContractError → Application
Network Error → NetworkError → Application
Invalid Input → ValidationError → Application
```

## Error Types

### 1. ValidationError

**When thrown:** Before making any contract call, when input validation fails.

**Properties:**
- `code`: Always `'VALIDATION_ERROR'`
- `field`: The name of the invalid field (optional)
- `message`: Description of what's invalid

**Examples:**
```typescript
// Empty program ID
ValidationError: Program ID cannot be empty (field: 'programId')

// Invalid address format
ValidationError: recipient is not a valid Stellar address (field: 'recipient')

// Zero or negative amount
ValidationError: Amount must be greater than zero (field: 'amount')

// Array length mismatch
ValidationError: Recipients and amounts arrays must have the same length (field: 'recipients')
```

### 2. ContractError

**When thrown:** When the smart contract execution fails or returns an error.

**Properties:**
- `code`: One of the `ContractErrorCode` enum values
- `contractErrorCode`: The numeric error code from the contract (optional)
- `message`: Human-readable error description

**Error Code Mapping:**

| Contract Panic Message | SDK Error Code | Description |
|------------------------|----------------|-------------|
| "Program not initialized" | `NOT_INITIALIZED` | Contract not initialized via `init_program` |
| "require_auth" failure | `UNAUTHORIZED` | Caller lacks required authorization |
| "Insufficient balance" | `INSUFFICIENT_BALANCE` | Not enough funds for the operation |
| "must be greater than zero" | `INVALID_AMOUNT` | Amount parameter is zero or negative |
| "already initialized" | `ALREADY_INITIALIZED` | Attempted to initialize twice |
| "empty batch" | `EMPTY_BATCH` | Batch payout with empty arrays |
| "same length" | `LENGTH_MISMATCH` | Recipients/amounts array length mismatch |
| "overflow" | `OVERFLOW` | Arithmetic overflow in calculations |

**Examples:**
```typescript
// Calling methods before initialization
ContractError: Program not initialized (code: NOT_INITIALIZED)

// Unauthorized payout attempt
ContractError: Unauthorized: caller does not have permission (code: UNAUTHORIZED)

// Insufficient funds
ContractError: Insufficient balance for this operation (code: INSUFFICIENT_BALANCE)
```

### 3. NetworkError

**When thrown:** When there are network, transport, or RPC server issues.

**Properties:**
- `code`: Always `'NETWORK_ERROR'`
- `statusCode`: HTTP status code (if applicable)
- `cause`: The original error that caused the network failure
- `message`: Description of the network issue

**Common Scenarios:**

| Scenario | Status Code | Message Pattern |
|----------|-------------|-----------------|
| Connection refused | - | "Failed to connect to RPC server: {url}" |
| Request timeout | - | "Failed to connect to RPC server: {url}" |
| Bad request | 400 | "RPC request failed with status 400" |
| Unauthorized | 401 | "RPC request failed with status 401" |
| Not found | 404 | "RPC request failed with status 404" |
| Server error | 500 | "RPC request failed with status 500" |
| Service unavailable | 503 | "RPC request failed with status 503" |

**Examples:**
```typescript
// RPC server down
NetworkError: Failed to connect to RPC server: https://soroban-testnet.stellar.org
  statusCode: undefined
  cause: Error { code: 'ECONNREFUSED' }

// Server error
NetworkError: RPC request failed with status 500
  statusCode: 500
  cause: Error { ... }
```

## Error Handling Best Practices

### 1. Validate Early

The SDK validates inputs before making contract calls to fail fast:

```typescript
// This throws ValidationError immediately, no contract call made
await client.lockProgramFunds(0n, keypair);
```

### 2. Handle Specific Error Types

```typescript
try {
  await client.singlePayout(recipient, amount, keypair);
} catch (error) {
  if (error instanceof ValidationError) {
    // Fix input and retry
    console.error('Invalid input:', error.field, error.message);
  } else if (error instanceof ContractError) {
    // Handle contract-specific errors
    switch (error.code) {
      case ContractErrorCode.NOT_INITIALIZED:
        // Initialize the program first
        break;
      case ContractErrorCode.UNAUTHORIZED:
        // Use correct keypair
        break;
      case ContractErrorCode.INSUFFICIENT_BALANCE:
        // Lock more funds
        break;
    }
  } else if (error instanceof NetworkError) {
    // Retry with backoff
    console.error('Network issue:', error.statusCode);
  }
}
```

### 3. Implement Retry Logic for Network Errors

```typescript
async function withRetry<T>(
  fn: () => Promise<T>,
  maxRetries: number = 3
): Promise<T> {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fn();
    } catch (error) {
      if (error instanceof NetworkError && i < maxRetries - 1) {
        await new Promise(resolve => setTimeout(resolve, 1000 * (i + 1)));
        continue;
      }
      throw error;
    }
  }
  throw new Error('Max retries exceeded');
}

// Usage
const balance = await withRetry(() => client.getRemainingBalance());
```

### 4. Log Errors Appropriately

```typescript
try {
  await client.batchPayout(recipients, amounts, keypair);
} catch (error) {
  if (error instanceof ValidationError) {
    // User error - log at info level
    logger.info('Validation failed', { field: error.field, message: error.message });
  } else if (error instanceof ContractError) {
    // Contract error - log at warning level
    logger.warn('Contract error', { code: error.code, message: error.message });
  } else if (error instanceof NetworkError) {
    // Network error - log at error level
    logger.error('Network error', { 
      statusCode: error.statusCode, 
      cause: error.cause?.message 
    });
  }
}
```

## Testing Error Scenarios

The SDK includes comprehensive tests for all error paths:

### Validation Error Tests
- Empty/invalid addresses
- Zero/negative amounts
- Empty arrays
- Array length mismatches

### Contract Error Tests
- All contract error codes
- Error message parsing
- Error factory functions

### Network Error Tests
- Connection failures (ECONNREFUSED, ETIMEDOUT)
- HTTP status codes (400, 401, 404, 500, 503)
- Error property preservation
- Retry scenarios

See `src/__tests__/error-handling.test.ts` and `src/__tests__/network-errors.test.ts` for complete test coverage.

## Future Enhancements

1. **Retry Middleware**: Built-in retry logic with exponential backoff
2. **Error Telemetry**: Automatic error reporting and metrics
3. **Custom Error Handlers**: Allow users to register custom error handlers
4. **Error Recovery Suggestions**: Provide actionable suggestions for each error type
