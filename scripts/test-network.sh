#!/bin/bash

# Test script for different networks
# Usage: ./scripts/test-network.sh [mainnet-beta|devnet|staging|localnet]

set -e

NETWORK=${1:-localnet}

echo "Testing saguaro-gatekeeper for network: $NETWORK"

# Update Anchor.toml cluster setting based on network
case $NETWORK in
  mainnet-beta)
    echo "🌐 Testing for mainnet-beta"
    # Update cluster to mainnet in Anchor.toml
    sed -i '' 's/cluster = .*/cluster = "mainnet"/' Anchor.toml
    # Build with mainnet features
    anchor build -- --features mainnet-beta
    ;;
  devnet)
    echo "🧪 Testing for devnet"
    # Update cluster to devnet in Anchor.toml
    sed -i '' 's/cluster = .*/cluster = "devnet"/' Anchor.toml
    # Build with devnet features
    anchor build -- --features devnet
    ;;
  staging)
    echo "🔧 Testing for staging"
    # Update cluster to devnet in Anchor.toml (staging uses devnet cluster)
    sed -i '' 's/cluster = .*/cluster = "devnet"/' Anchor.toml
    # Build with staging features
    anchor build -- --features staging
    ;;
  localnet)
    echo "🏠 Testing for localnet"
    # Update cluster to localnet in Anchor.toml
    sed -i '' 's/cluster = .*/cluster = "localnet"/' Anchor.toml
    # Build with default (localnet)
    anchor build
    ;;
  *)
    echo "❌ Unknown network: $NETWORK"
    echo "Usage: $0 [mainnet-beta|devnet|staging|localnet]"
    exit 1
    ;;
esac

echo "📦 Running tests..."

# Set environment variables based on network
case $NETWORK in
  mainnet-beta)
    export ANCHOR_PROVIDER_URL="https://api.mainnet-beta.solana.com"
    ;;
  devnet)
    export ANCHOR_PROVIDER_URL="https://api.devnet.solana.com"
    ;;
  staging)
    export ANCHOR_PROVIDER_URL="https://api.devnet.solana.com"  # staging uses devnet
    ;;
  localnet)
    export ANCHOR_PROVIDER_URL="http://127.0.0.1:8899"
    ;;
esac

# Set wallet from Anchor.toml or default
WALLET_PATH=${ANCHOR_WALLET:-"$HOME/.config/solana/id.json"}
export ANCHOR_WALLET="$WALLET_PATH"

echo "Using provider URL: $ANCHOR_PROVIDER_URL"
echo "Using wallet: $ANCHOR_WALLET"

# Run tests (this will use the already built program)
yarn run ts-mocha -p ./tsconfig.json -t 1000000 'tests/**/*.ts'

echo "✅ Tests completed for $NETWORK"