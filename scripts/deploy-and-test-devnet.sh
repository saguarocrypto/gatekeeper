#!/bin/bash

# Deploy and Test Saguaro Gatekeeper on Devnet
# This script handles the complete deployment and testing process

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color
BOLD='\033[1m'

echo -e "${BOLD}${BLUE}=== Saguaro Gatekeeper Devnet Deployment & Testing ===${NC}\n"

# Configuration
NETWORK="devnet"
WALLET_PATH="${WALLET_PATH:-$HOME/.config/solana/id.json}"
PROGRAM_NAME="saguaro_gatekeeper"
PROGRAM_PATH="./target/deploy/${PROGRAM_NAME}.so"
KEYPAIR_PATH="./target/deploy/${PROGRAM_NAME}-keypair.json"

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
echo -e "${CYAN}Checking prerequisites...${NC}"

if ! command_exists solana; then
    echo -e "${RED}Error: solana CLI not found. Please install Solana CLI.${NC}"
    exit 1
fi

if ! command_exists anchor; then
    echo -e "${RED}Error: anchor CLI not found. Please install Anchor framework.${NC}"
    exit 1
fi

if ! command_exists node; then
    echo -e "${RED}Error: node not found. Please install Node.js.${NC}"
    exit 1
fi

# Check wallet exists
if [ ! -f "$WALLET_PATH" ]; then
    echo -e "${RED}Error: Wallet not found at $WALLET_PATH${NC}"
    echo -e "${YELLOW}Please create a wallet or set WALLET_PATH environment variable${NC}"
    exit 1
fi

# Set Solana configuration
echo -e "\n${CYAN}Configuring Solana for devnet...${NC}"
solana config set --url https://api.devnet.solana.com
solana config set --keypair "$WALLET_PATH"

# Get wallet address and balance
WALLET_ADDRESS=$(solana address)
echo -e "${YELLOW}Wallet address: $WALLET_ADDRESS${NC}"
echo -e "${BLUE}📋 Wallet Explorer: https://explorer.solana.com/address/$WALLET_ADDRESS?cluster=devnet${NC}"

BALANCE=$(solana balance --url devnet | awk '{print $1}')
echo -e "${YELLOW}Wallet balance: $BALANCE SOL${NC}"

# Check if balance is sufficient
if (( $(echo "$BALANCE < 1" | bc -l) )); then
    echo -e "${YELLOW}Low balance detected. Requesting airdrop...${NC}"
    solana airdrop 5 --url devnet || {
        echo -e "${RED}Airdrop failed. Please try again or fund your wallet manually.${NC}"
        exit 1
    }
    sleep 5  # Wait for airdrop to confirm
    BALANCE=$(solana balance --url devnet | awk '{print $1}')
    echo -e "${GREEN}New balance: $BALANCE SOL${NC}"
fi

# Build the program
echo -e "\n${CYAN}Building program...${NC}"
anchor build

# Get or generate program keypair
if [ -f "$KEYPAIR_PATH" ]; then
    echo -e "${YELLOW}Using existing program keypair${NC}"
    PROGRAM_ID=$(solana address -k "$KEYPAIR_PATH")
else
    echo -e "${YELLOW}Generating new program keypair...${NC}"
    solana keygen new -o "$KEYPAIR_PATH" --no-passphrase --force
    PROGRAM_ID=$(solana address -k "$KEYPAIR_PATH")
fi

echo -e "${YELLOW}Program ID: $PROGRAM_ID${NC}"
echo -e "${BLUE}📋 Program Explorer: https://explorer.solana.com/address/$PROGRAM_ID?cluster=devnet${NC}"

# Update program ID in Anchor.toml and lib.rs
echo -e "\n${CYAN}Updating program ID in project files...${NC}"

# Update Anchor.toml
if [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    sed -i '' "s/^${PROGRAM_NAME} = \".*\"$/${PROGRAM_NAME} = \"$PROGRAM_ID\"/" Anchor.toml
    sed -i '' "s/^cluster = \".*\"$/cluster = \"devnet\"/" Anchor.toml
else
    # Linux
    sed -i "s/^${PROGRAM_NAME} = \".*\"$/${PROGRAM_NAME} = \"$PROGRAM_ID\"/" Anchor.toml
    sed -i "s/^cluster = \".*\"$/cluster = \"devnet\"/" Anchor.toml
fi

# Update lib.rs declare_id
LIB_RS_PATH="programs/saguaro-gatekeeper/src/lib.rs"
if [[ "$OSTYPE" == "darwin"* ]]; then
    sed -i '' "s/declare_id!(\".*\");/declare_id!(\"$PROGRAM_ID\");/" "$LIB_RS_PATH"
else
    sed -i "s/declare_id!(\".*\");/declare_id!(\"$PROGRAM_ID\");/" "$LIB_RS_PATH"
fi

# Rebuild with updated program ID
echo -e "\n${CYAN}Rebuilding with updated program ID...${NC}"
anchor build

# Deploy the program
echo -e "\n${CYAN}Deploying program to devnet...${NC}"
echo -e "${YELLOW}This may take a few minutes...${NC}"

# Capture deployment with better visibility
echo -e "${YELLOW}Deploying to program address: $PROGRAM_ID${NC}"
echo -e "${BLUE}📋 Track Deployment: https://explorer.solana.com/address/$PROGRAM_ID?cluster=devnet${NC}"

anchor deploy --provider.cluster devnet --provider.wallet "$WALLET_PATH" || {
    echo -e "${RED}Deployment failed!${NC}"
    echo -e "${YELLOW}If you see 'account data too small' error, try:${NC}"
    echo -e "  solana program extend $PROGRAM_ID 20000 --url devnet"
    echo -e "${BLUE}📋 Check program status: https://explorer.solana.com/address/$PROGRAM_ID?cluster=devnet${NC}"
    exit 1
}

echo -e "${GREEN}✓ Program deployed successfully!${NC}"
echo -e "${BLUE}📋 View Deployment: https://explorer.solana.com/address/$PROGRAM_ID?cluster=devnet${NC}"

# Verify deployment
echo -e "\n${CYAN}Verifying deployment...${NC}"
DEPLOYED_PROGRAM=$(solana program show $PROGRAM_ID --url devnet 2>/dev/null | grep "Program Id:" | awk '{print $3}')

if [ "$DEPLOYED_PROGRAM" == "$PROGRAM_ID" ]; then
    echo -e "${GREEN}✓ Program verified on devnet${NC}"
    echo -e "${BLUE}📋 Verified Program: https://explorer.solana.com/address/$PROGRAM_ID?cluster=devnet${NC}"
else
    echo -e "${RED}Error: Could not verify program deployment${NC}"
    exit 1
fi

# Install dependencies if needed
if [ ! -d "node_modules" ]; then
    echo -e "\n${CYAN}Installing dependencies...${NC}"
    yarn install || npm install
fi

# Run the tests
echo -e "\n${CYAN}Running devnet tests...${NC}"
echo -e "${YELLOW}Program ID: $PROGRAM_ID${NC}"
echo -e "${BLUE}📋 Monitor Program: https://explorer.solana.com/address/$PROGRAM_ID?cluster=devnet${NC}"

# Export program ID for the test script
export PROGRAM_ID
export ANCHOR_PROVIDER_URL="https://api.devnet.solana.com"
export ANCHOR_WALLET="$WALLET_PATH"

# Run the TypeScript test file
npx ts-node scripts/devnet-tests.ts

# Check test results
TEST_EXIT_CODE=$?

if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo -e "\n${GREEN}${BOLD}✓ All tests passed successfully! 🎉${NC}"
    echo -e "\n${CYAN}Program Details:${NC}"
    echo -e "  ${YELLOW}Program ID:${NC} $PROGRAM_ID"
    echo -e "  ${YELLOW}Network:${NC} Devnet"
    echo -e "  ${BLUE}📋 Program Explorer:${NC} https://explorer.solana.com/address/$PROGRAM_ID?cluster=devnet"
    echo -e "  ${BLUE}📋 Wallet Explorer:${NC} https://explorer.solana.com/address/$WALLET_ADDRESS?cluster=devnet"
else
    echo -e "\n${RED}${BOLD}✗ Tests failed!${NC}"
    exit $TEST_EXIT_CODE
fi

echo -e "\n${CYAN}To run tests again without redeploying:${NC}"
echo -e "  PROGRAM_ID=$PROGRAM_ID npx ts-node scripts/devnet-tests.ts"

echo -e "\n${CYAN}To view program logs:${NC}"
echo -e "  solana logs $PROGRAM_ID --url devnet"

echo -e "\n${CYAN}Useful Links:${NC}"
echo -e "  ${BLUE}📋 Program Logs:${NC} https://explorer.solana.com/address/$PROGRAM_ID/logs?cluster=devnet"
echo -e "  ${BLUE}📋 Program Instructions:${NC} https://explorer.solana.com/address/$PROGRAM_ID/instruction-logs?cluster=devnet"
echo -e "  ${BLUE}📋 All Transactions:${NC} https://explorer.solana.com/address/$WALLET_ADDRESS?cluster=devnet"