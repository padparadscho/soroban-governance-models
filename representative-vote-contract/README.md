# Representative Vote Contract

This contract implements a "**delegated voting**" democratic governance model where token holders assign their voting power to representatives who vote on their behalf, enabling scalable governance while maintaining democratic principles.

## Overview

**Voting Process:**

- **Delegation Verification:** Token holders delegate their voting power to approved representatives.
- **Weight Assignment:** Each delegate's vote carries weight equal to their accumulated delegated token balance.
- **Duplicate Prevention:** The contract enforces one delegation per holder per proposal.
- **Vote Aggregation:** Tallies accumulate using representative weighted calculations.
- **Overflow Protection:** Uses saturating arithmetic to prevent vote count manipulation.

**Proposal Lifecycle:**

- **Creation:** Admin creates proposals with time validation (5 to 15-day duration limits).
- **Delegation Period:** Token holders delegate voting power to representatives during the active time window.
- **Vote Counting:** Each delegate vote counts with weight equal to their accumulated delegated tokens.
- **Resolution:** A representative weighted majority determines the outcome.

## Getting Started

### Prerequisites

- **Rust & Soroban Environment**: Set up the environment for building, deploying, and interacting with Soroban contracts. Detailed instructions are available in the [Stellar Developers Documentation](https://developers.stellar.org/docs/build/smart-contracts/getting-started/setup).

- **Stellar Asset Contract (SAC)**: Deploy the SAC for the Stellar asset intended to be used in the contract using the Stellar CLI. Refer to the [Deploy the Stellar Asset Contract for a Stellar asset](https://developers.stellar.org/docs/build/guides/cli/deploy-stellar-asset-contract) guide for instructions.

### Testing

- Run the complete test suite:

  ```bash
  cargo test
  ```

- For verbose output:

  ```bash
  cargo test -- --nocapture
  ```

- Run a specific test:

  ```bash
  cargo test test_vote
  ```

### Usage

- **Build**: Compile contract to WASM for deployment.

  ```bash
  stellar contract build
  ```

- `__constructor`: Deploy and initialize with admin and token addresses.

  ```bash
  stellar contract deploy \
  --wasm target/wasm32v1-none/release/representative_vote_contract.wasm \
  --alias representative-vote-contract \
  --source <DEPLOYER_PRIVATE_KEY> \
  --network testnet \
  -- \
  --admin <ADMIN_PUBLIC_KEY> \
  --token <STELLAR_ASSET_CONTRACT>
  ```

- `set_delegates`: Set the complete list of approved delegates (admin only).

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <ADMIN_PRIVATE_KEY> \
  --network testnet \
  -- \
  set_delegates \
  --delegates <'["DELEGATE_1_PUBLIC_KEY", "DELEGATE_2_PUBLIC_KEY"]'>
  ```

- `add_delegates`: Add new addresses to the approved delegates list (admin only).

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <ADMIN_PRIVATE_KEY> \
  --network testnet \
  -- \
  add_delegates \
  --to_add <'["DELEGATE_PUBLIC_KEY"]'>
  ```

- `remove_delegates`: Remove addresses from the approved delegates list (admin only).

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <ADMIN_PRIVATE_KEY> \
  --network testnet \
  -- \
  remove_delegates \
  --to_remove <'["DELEGATE_PUBLIC_KEY"]'>
  ```

- `create_proposal`: Create new proposal with delegation deadline and voting window (admin only, 5-15 day duration).

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <ADMIN_PRIVATE_KEY> \
  --network testnet \
  -- \
  create_proposal \
  --id <"SYMBOL"> \
  --description <"STRING"> \
  --delegation_deadline <UNIX_TIMESTAMP> \
  --start_time <UNIX_TIMESTAMP> \
  --end_time <UNIX_TIMESTAMP>
  ```

- `delegate`: Token holder delegates voting power to an approved representative.

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  delegate \
  --user <CALLER_PUBLIC_KEY> \
  --id <"SYMBOL"> \
  --delegate_address <DELEGATE_PUBLIC_KEY>
  ```

- `revoke_delegation`: Revoke delegation within the 48-hour grace period.

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  revoke_delegation \
  --user <CALLER_PUBLIC_KEY> \
  --id <"SYMBOL">
  ```

- `vote`: Cast vote as a delegate using accumulated delegated power.

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <DELEGATE_PRIVATE_KEY> \
  --network testnet \
  -- \
  vote \
  --delegate <DELEGATE_PUBLIC_KEY> \
  --id <"SYMBOL"> \
  --choice <"SYMBOL">
  ```

- `transfer_admin`: Transfer admin privileges (current admin only).

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <ADMIN_PRIVATE_KEY> \
  --network testnet \
  -- \
  transfer_admin \
  --new_admin <NEW_ADMIN_PUBLIC_KEY>
  ```

- `get_governance_details`: Get all proposal summaries.

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  get_governance_details
  ```

- `get_proposal_details`: Get specific proposal data including vote counts.

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  get_proposal_details \
  --id <"SYMBOL">
  ```

- `get_user_details`: Get user delegation history and eligibility.

  ```bash
  stellar contract invoke \
  --id representative-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  get_user_details \
  --user <CALLER_PUBLIC_KEY>
  ```
