# Saguaro Gatekeeper Devnet Testing Scripts

This directory contains scripts for deploying and testing the Saguaro Gatekeeper program on Solana devnet.

## Scripts Overview

### 1. `deploy-and-test-devnet.sh`
Full deployment and testing script that:
- Checks prerequisites
- Builds the program
- Deploys to devnet
- Runs comprehensive tests

Usage:
```bash
./scripts/deploy-and-test-devnet.sh
```

### 2. `run-devnet-tests.sh`
Runs tests against an already deployed program:

Usage:
```bash
./scripts/run-devnet-tests.sh <PROGRAM_ID>
# or
PROGRAM_ID=<PROGRAM_ID> ./scripts/run-devnet-tests.sh
```

### 3. `test-devnet-simple.sh`
Runs simplified tests for quick verification:

Usage:
```bash
./scripts/test-devnet-simple.sh <PROGRAM_ID>
```

### 4. `deploy-fresh-devnet.sh`
Deploys a fresh instance with a new program ID:

Usage:
```bash
./scripts/deploy-fresh-devnet.sh
```

### 5. `update-program-id.sh`
Updates the program ID in source files:

Usage:
```bash
./scripts/update-program-id.sh <NEW_PROGRAM_ID>
```

## Test Files

### `devnet-tests.ts`
Comprehensive test suite that covers:
- Creating sandwich validators PDAs
- Authorization checks
- Slot validation
- Updating validators
- Closing PDAs
- Large bitmap operations

### `devnet-tests-simple.ts`
Simplified test suite for quick verification of basic functionality.

## Prerequisites

1. Solana CLI installed
2. Anchor framework installed
3. Node.js and Yarn/NPM
4. Funded wallet (at least 2 SOL for deployment)

## Common Issues

### Program ID Mismatch
If you see "DeclaredProgramIdMismatch" errors, it means the deployed program has a different ID in its code than expected. Solutions:
1. Deploy a fresh instance using `deploy-fresh-devnet.sh`
2. Update your local program ID using `update-program-id.sh`

### Low Balance
If your wallet has insufficient SOL:
```bash
solana airdrop 2 --url devnet
```

### View Logs
To see program logs:
```bash
solana logs <PROGRAM_ID> --url devnet
```

## Environment Variables

- `WALLET_PATH`: Path to your Solana wallet (default: `~/.config/solana/id.json`)
- `PROGRAM_ID`: Program ID to test against
- `ANCHOR_PROVIDER_URL`: RPC URL (automatically set to devnet)
- `ANCHOR_WALLET`: Wallet path for Anchor