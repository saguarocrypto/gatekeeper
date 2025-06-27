#!/bin/bash

# Deploy a fresh instance of the program to devnet
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'
BOLD='\033[1m'

# Check for --test-only flag
TEST_ONLY=false
if [[ "$1" == "--test-only" ]]; then
    TEST_ONLY=true
    echo -e "${BOLD}${BLUE}=== Running Tests Only (No Deployment) ===${NC}\n"
else
    echo -e "${BOLD}${BLUE}=== Fresh Deployment to Devnet ===${NC}\n"
fi

# Configuration
WALLET_PATH="${WALLET_PATH:-$HOME/.config/solana/id.json}"

# Check wallet
if [ ! -f "$WALLET_PATH" ]; then
    echo -e "${RED}Error: Wallet not found at $WALLET_PATH${NC}"
    exit 1
fi

if [ "$TEST_ONLY" = false ]; then
    # Set Solana configuration
    echo -e "${CYAN}Configuring Solana for devnet...${NC}"
    solana config set --url https://api.devnet.solana.com
    solana config set --keypair "$WALLET_PATH"

    # Check balance
    BALANCE=$(solana balance --url devnet | awk '{print $1}')
    echo -e "${YELLOW}Wallet balance: $BALANCE SOL${NC}"

    if (( $(echo "$BALANCE < 2" | bc -l) )); then
        echo -e "${YELLOW}Low balance. Requesting airdrop...${NC}"
        solana airdrop 2 --url devnet
        sleep 5
    fi

    # Generate new program keypair
    echo -e "\n${CYAN}Generating new program keypair...${NC}"
    KEYPAIR_PATH="./target/deploy/saguaro_gatekeeper-keypair-new.json"
    solana-keygen new -o "$KEYPAIR_PATH" --no-bip39-passphrase --force
    NEW_PROGRAM_ID=$(solana address -k "$KEYPAIR_PATH")

    echo -e "${GREEN}New Program ID: $NEW_PROGRAM_ID${NC}"

    # Update program ID in source
    echo -e "\n${CYAN}Updating program ID in source files...${NC}"
    ./scripts/update-program-id.sh "$NEW_PROGRAM_ID"

    # Build the program
    echo -e "\n${CYAN}Building program...${NC}"
    anchor build

    # Deploy
    echo -e "\n${CYAN}Deploying to devnet...${NC}"
    solana program deploy \
        --url https://api.devnet.solana.com \
        --keypair "$WALLET_PATH" \
        --program-id "$KEYPAIR_PATH" \
        ./target/deploy/saguaro_gatekeeper.so

    echo -e "\n${GREEN}✓ Deployment successful!${NC}"
    echo -e "${YELLOW}Program ID: $NEW_PROGRAM_ID${NC}"
    echo -e "${CYAN}Explorer: https://explorer.solana.com/address/$NEW_PROGRAM_ID?cluster=devnet${NC}"

    # Run tests
    echo -e "\n${CYAN}Running tests on fresh deployment...${NC}"
    export PROGRAM_ID="$NEW_PROGRAM_ID"
    ./scripts/test-devnet-simple.sh "$NEW_PROGRAM_ID"
else
    # Test-only mode: get program ID from Anchor.toml
    CURRENT_PROGRAM_ID=$(grep -A 1 "\[programs.devnet\]" Anchor.toml | grep "saguaro_gatekeeper" | cut -d'"' -f2)
    if [ -z "$CURRENT_PROGRAM_ID" ]; then
        echo -e "${RED}Error: Could not find program ID in Anchor.toml${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}Using existing Program ID: $CURRENT_PROGRAM_ID${NC}"
    echo -e "${CYAN}Explorer: https://explorer.solana.com/address/$CURRENT_PROGRAM_ID?cluster=devnet${NC}"
    
    # Run tests
    echo -e "\n${CYAN}Running tests on existing deployment...${NC}"
    export PROGRAM_ID="$CURRENT_PROGRAM_ID"
    ./scripts/test-devnet-simple.sh "$CURRENT_PROGRAM_ID"
fi