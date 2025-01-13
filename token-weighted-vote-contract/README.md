# Token-Weighted Vote Contract

This contract implements a "**token balance equals voting power**" plutocratic governance model where each user's vote carries weight proportional to their token balance, creating a system where economic stake determines governance power.

## Overview

**Voting Process:**

- **Token Verification:** Users must hold any amount > 0 of the governance token to participate.
- **Weight Assignment:** Each vote's weight is proportional to the holder's token balance.
- **Duplicate Prevention:** The contract enforces one vote per holder per proposal.
- **Vote Aggregation:** Tallies accumulate using token-weighted calculations.
- **Overflow Protection:** Uses saturating arithmetic to prevent vote count manipulation.

**Proposal Lifecycle:**

- **Creation:** Admin creates proposals with time validation (5-15 day duration limits).
- **Voting Period:** Token holders cast votes during the active time window.
- **Vote Counting:** Each vote counts with weight equal to holder's token balance.
- **Resolution:** Token-weighted majority determines outcome.

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
  --wasm target/wasm32v1-none/release/token_weighted_vote_contract.wasm \
  --alias token-weighted-vote-contract \
  --source <DEPLOYER_PRIVATE_KEY> \
  --network testnet \
  -- \
  --admin <ADMIN_PUBLIC_KEY> \
  --token <STELLAR_ASSET_CONTRACT>
  ```

- `create_proposal`: Create new proposal (admin only, 5-15 day duration).

  ```bash
  stellar contract invoke \
  --id token-weighted-vote-contract \
  --source <ADMIN_PRIVATE_KEY> \
  --network testnet \
  -- \
  create_proposal \
  --id <"SYMBOL"> \
  --description <"STRING"> \
  --start_time <UNIX_TIMESTAMP> \
  --end_time <UNIX_TIMESTAMP>
  ```

- `vote`: Cast vote (requires token balance > 0, weight equals balance).

  ```bash
  stellar contract invoke \
  --id token-weighted-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  vote \
  --user <CALLER_PUBLIC_KEY> \
  --id <"SYMBOL"> \
  --choice <"SYMBOL">
  ```

- `transfer_admin`: Transfer admin privileges (current admin only).

  ```bash
  stellar contract invoke \
  --id token-weighted-vote-contract \
  --source <ADMIN_PRIVATE_KEY> \
  --network testnet \
  -- \
  transfer_admin \
  --new_admin <NEW_ADMIN_PUBLIC_KEY>
  ```

- `get_governance_details`: Get all proposal summaries.

  ```bash
  stellar contract invoke \
  --id token-weighted-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  get_governance_details
  ```

- `get_proposal_details`: Get specific proposal data including vote counts.

  ```bash
  stellar contract invoke \
  --id token-weighted-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  get_proposal_details \
  --id <"SYMBOL">
  ```

- `get_user_details`: Get user voting history and eligibility.

  ```bash
  stellar contract invoke \
  --id token-weighted-vote-contract \
  --source <CALLER_PRIVATE_KEY> \
  --network testnet \
  -- \
  get_user_details \
  --user <CALLER_PUBLIC_KEY>
  ```
