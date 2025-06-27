#!/bin/bash

# Run Saguaro Gatekeeper Tests on Devnet
# This script runs tests against an already deployed program

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

echo -e "${BOLD}${BLUE}=== Saguaro Gatekeeper Devnet Test Runner ===${NC}\n"

# Check if PROGRAM_ID is provided
if [ -z "$1" ] && [ -z "$PROGRAM_ID" ]; then
    echo -e "${RED}Error: Program ID not provided${NC}"
    echo -e "${YELLOW}Usage: $0 <PROGRAM_ID>${NC}"
    echo -e "${YELLOW}Or set PROGRAM_ID environment variable${NC}"
    exit 1
fi

# Use provided program ID or environment variable
PROGRAM_ID="${1:-$PROGRAM_ID}"
WALLET_PATH="${WALLET_PATH:-$HOME/.config/solana/id.json}"

echo -e "${CYAN}Configuration:${NC}"
echo -e "  ${YELLOW}Program ID:${NC} $PROGRAM_ID"
echo -e "  ${YELLOW}Wallet:${NC} $WALLET_PATH"
echo -e "  ${YELLOW}Network:${NC} Devnet"

# Check wallet exists
if [ ! -f "$WALLET_PATH" ]; then
    echo -e "${RED}Error: Wallet not found at $WALLET_PATH${NC}"
    exit 1
fi

# Check balance
echo -e "\n${CYAN}Checking wallet balance...${NC}"
BALANCE=$(solana balance --url devnet | awk '{print $1}')
echo -e "${YELLOW}Balance: $BALANCE SOL${NC}"

if (( $(echo "$BALANCE < 0.5" | bc -l) )); then
    echo -e "${RED}Warning: Low balance! Tests may fail.${NC}"
    echo -e "${YELLOW}Consider running: solana airdrop 2 --url devnet${NC}"
fi

# Verify program exists on devnet
echo -e "\n${CYAN}Verifying program on devnet...${NC}"
if solana program show $PROGRAM_ID --url devnet &>/dev/null; then
    echo -e "${GREEN}✓ Program found on devnet${NC}"
else
    echo -e "${RED}Error: Program not found on devnet${NC}"
    echo -e "${YELLOW}Please deploy the program first using deploy-and-test-devnet.sh${NC}"
    exit 1
fi

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
    echo -e "\n${CYAN}Installing dependencies...${NC}"
    yarn install || npm install
fi

# Run the tests
echo -e "\n${CYAN}Running tests...${NC}"

# Export environment variables
export PROGRAM_ID
export ANCHOR_PROVIDER_URL="https://api.devnet.solana.com"
export ANCHOR_WALLET="$WALLET_PATH"
export WALLET_PATH

# Run the TypeScript test file
npx ts-node scripts/devnet-tests.ts

# Check test results
TEST_EXIT_CODE=$?

if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo -e "\n${GREEN}${BOLD}✓ All tests passed! 🎉${NC}"
else
    echo -e "\n${RED}${BOLD}✗ Tests failed!${NC}"
    exit $TEST_EXIT_CODE
fi

echo -e "\n${CYAN}View program on Solana Explorer:${NC}"
echo -e "  https://explorer.solana.com/address/$PROGRAM_ID?cluster=devnet"

echo -e "\n${CYAN}View program logs:${NC}"
echo -e "  solana logs $PROGRAM_ID --url devnet"