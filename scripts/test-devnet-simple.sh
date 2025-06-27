#!/bin/bash

# Simple devnet test runner
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}=== Running Simple Devnet Tests ===${NC}"

# Set environment
export ANCHOR_PROVIDER_URL="https://api.devnet.solana.com"
export ANCHOR_WALLET="${WALLET_PATH:-$HOME/.config/solana/id.json}"

# If program ID is provided, use it
if [ ! -z "$1" ]; then
    export PROGRAM_ID="$1"
    echo -e "${YELLOW}Using Program ID: $PROGRAM_ID${NC}"
fi

# Run the simple test
npx ts-node scripts/devnet-tests-simple.ts