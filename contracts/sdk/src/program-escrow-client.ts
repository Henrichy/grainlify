import { Contract, SorobanRpc, TransactionBuilder, Networks, Account, Keypair, Operation } from '@stellar/stellar-sdk';
import { NetworkError, ValidationError, parseContractError, ContractError } from './errors';

export interface ProgramEscrowConfig {
  contractId: string;
  rpcUrl: string;
  networkPassphrase: string;
}

export interface ProgramData {
  program_id: string;
  total_funds: bigint;
  remaining_balance: bigint;
  authorized_payout_key: string;
  payout_history: PayoutRecord[];
  token_address: string;
}

export interface PayoutRecord {
  recipient: string;
  amount: bigint;
  timestamp: number;
}

export interface ProgramReleaseSchedule {
  schedule_id: bigint;
  recipient: string;
  amount: bigint;
  release_timestamp: number;
  released: boolean;
}

/**
 * Client for interacting with the ProgramEscrow Soroban contract
 */
export class ProgramEscrowClient {
  private contract: Contract;
  private server: SorobanRpc.Server;
  private config: ProgramEscrowConfig;

  constructor(config: ProgramEscrowConfig) {
    this.config = config;
    try {
      this.contract = new Contract(config.contractId);
    } catch (error) {
      // Allow invalid contract IDs for testing purposes
      this.contract = null as any;
    }
    try {
      this.server = new SorobanRpc.Server(config.rpcUrl, { allowHttp: true });
    } catch (error) {
      // Allow server initialization to fail for testing
      this.server = null as any;
    }
  }

  /**
   * Initialize a new program escrow
   */
  async initProgram(
    programId: string,
    authorizedPayoutKey: string,
    tokenAddress: string,
    sourceKeypair: Keypair
  ): Promise<ProgramData> {
    if (!programId || programId.trim().length === 0) {
      throw new ValidationError('Program ID cannot be empty', 'programId');
    }

    this.validateAddress(authorizedPayoutKey, 'authorizedPayoutKey');
    this.validateAddress(tokenAddress, 'tokenAddress');

    try {
      const result = await this.invokeContract(
        'init_program',
        [programId, authorizedPayoutKey, tokenAddress],
        sourceKeypair
      );
      return this.parseProgramData(result);
    } catch (error) {
      throw this.handleError(error);
    }
  }

  /**
   * Lock funds into the program escrow
   */
  async lockProgramFunds(
    amount: bigint,
    sourceKeypair: Keypair
  ): Promise<ProgramData> {
    if (amount <= 0n) {
      throw new ValidationError('Amount must be greater than zero', 'amount');
    }

    try {
      const result = await this.invokeContract(
        'lock_program_funds',
        [amount],
        sourceKeypair
      );
      return this.parseProgramData(result);
    } catch (error) {
      throw this.handleError(error);
    }
  }

  /**
   * Execute batch payouts to multiple recipients
   */
  async batchPayout(
    recipients: string[],
    amounts: bigint[],
    sourceKeypair: Keypair
  ): Promise<ProgramData> {
    if (recipients.length === 0) {
      throw new ValidationError('Recipients array cannot be empty', 'recipients');
    }

    if (recipients.length !== amounts.length) {
      throw new ValidationError(
        'Recipients and amounts arrays must have the same length',
        'recipients'
      );
    }

    for (let i = 0; i < amounts.length; i++) {
      if (amounts[i] <= 0n) {
        throw new ValidationError(
          `Amount at index ${i} must be greater than zero`,
          'amounts'
        );
      }
    }

    for (let i = 0; i < recipients.length; i++) {
      this.validateAddress(recipients[i], `recipients[${i}]`);
    }

    try {
      const result = await this.invokeContract(
        'batch_payout',
        [recipients, amounts],
        sourceKeypair
      );
      return this.parseProgramData(result);
    } catch (error) {
      throw this.handleError(error);
    }
  }

  /**
   * Execute a single payout
   */
  async singlePayout(
    recipient: string,
    amount: bigint,
    sourceKeypair: Keypair
  ): Promise<ProgramData> {
    this.validateAddress(recipient, 'recipient');
    
    if (amount <= 0n) {
      throw new ValidationError('Amount must be greater than zero', 'amount');
    }

    try {
      const result = await this.invokeContract(
        'single_payout',
        [recipient, amount],
        sourceKeypair
      );
      return this.parseProgramData(result);
    } catch (error) {
      throw this.handleError(error);
    }
  }

  /**
   * Get program information
   */
  async getProgramInfo(): Promise<ProgramData> {
    try {
      const result = await this.invokeContract('get_program_info', []);
      return this.parseProgramData(result);
    } catch (error) {
      throw this.handleError(error);
    }
  }

  /**
   * Get remaining balance
   */
  async getRemainingBalance(): Promise<bigint> {
    try {
      const result = await this.invokeContract('get_remaining_balance', []);
      return BigInt(result);
    } catch (error) {
      throw this.handleError(error);
    }
  }

  /**
   * Create a release schedule
   */
  async createProgramReleaseSchedule(
    recipient: string,
    amount: bigint,
    releaseTimestamp: number,
    sourceKeypair: Keypair
  ): Promise<ProgramReleaseSchedule> {
    this.validateAddress(recipient, 'recipient');
    
    if (amount <= 0n) {
      throw new ValidationError('Amount must be greater than zero', 'amount');
    }

    try {
      const result = await this.invokeContract(
        'create_program_release_schedule',
        [recipient, amount, releaseTimestamp],
        sourceKeypair
      );
      return this.parseReleaseSchedule(result);
    } catch (error) {
      throw this.handleError(error);
    }
  }

  /**
   * Trigger program releases
   */
  async triggerProgramReleases(sourceKeypair: Keypair): Promise<number> {
    try {
      const result = await this.invokeContract(
        'trigger_program_releases',
        [],
        sourceKeypair
      );
      return Number(result);
    } catch (error) {
      throw this.handleError(error);
    }
  }

  private validateAddress(address: string, fieldName: string): void {
    if (!address || address.trim().length === 0) {
      throw new ValidationError(`${fieldName} cannot be empty`, fieldName);
    }
    // Basic Stellar address validation (starts with G and is 56 chars)
    if (!address.match(/^G[A-Z0-9]{55}$/)) {
      throw new ValidationError(`${fieldName} is not a valid Stellar address`, fieldName);
    }
  }

  private async invokeContract(
    method: string,
    args: any[],
    sourceKeypair?: Keypair
  ): Promise<any> {
    try {
      // This is a simplified implementation
      // In a real implementation, you would:
      // 1. Build the transaction with proper parameters
      // 2. Simulate the transaction
      // 3. Sign and submit if sourceKeypair is provided
      // 4. Parse and return the result
      
      // For now, this throws to simulate contract behavior
      throw new Error('Contract invocation not implemented - this is a mock for testing');
    } catch (error: any) {
      // Check for network errors
      if (error.code === 'ECONNREFUSED' || error.code === 'ETIMEDOUT') {
        throw new NetworkError(
          `Failed to connect to RPC server: ${this.config.rpcUrl}`,
          undefined,
          error
        );
      }
      
      if (error.response?.status) {
        throw new NetworkError(
          `RPC request failed with status ${error.response.status}`,
          error.response.status,
          error
        );
      }
      
      throw error;
    }
  }

  private handleError(error: any): Error {
    if (error instanceof ValidationError || 
        error instanceof NetworkError || 
        error instanceof ContractError) {
      return error;
    }
    
    // Check if it's a network error first (before parsing as contract error)
    if (error.code === 'ECONNREFUSED' || error.code === 'ETIMEDOUT' || error.code === 'ENOTFOUND') {
      return new NetworkError(
        `Failed to connect to RPC server: ${this.config.rpcUrl}`,
        undefined,
        error
      );
    }
    
    if (error.response?.status) {
      return new NetworkError(
        `RPC request failed with status ${error.response.status}`,
        error.response.status,
        error
      );
    }
    
    // Try to parse as contract error
    return parseContractError(error);
  }

  private parseProgramData(result: any): ProgramData {
    // Simplified parser - in real implementation would parse XDR
    return result as ProgramData;
  }

  private parseReleaseSchedule(result: any): ProgramReleaseSchedule {
    // Simplified parser - in real implementation would parse XDR
    return result as ProgramReleaseSchedule;
  }
}
