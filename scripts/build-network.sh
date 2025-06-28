#!/bin/bash

# Build script for different networks
# Usage: ./scripts/build-network.sh [mainnet-beta|devnet|staging|localnet]

set -e

NETWORK=${1:-localnet}

echo "Building saguaro-gatekeeper for network: $NETWORK"

case $NETWORK in
  mainnet-beta)
    echo "🌐 Building for mainnet-beta (saGUaroo4mjAcckhEPhtSRthGgFLdQpBvQvuwdf7YG3)"
    anchor build -- --features mainnet-beta
    ;;
  devnet)
    echo "🧪 Building for devnet (D291aiDeFSTj1KVJ1ScwTpCFnALaPkNqDQCduGdDBTHg)"
    anchor build -- --features devnet
    ;;
  staging)
    echo "🔧 Building for staging (D291aiDeFSTj1KVJ1ScwTpCFnALaPkNqDQCduGdDBTHg)"
    anchor build -- --features staging
    ;;
  localnet)
    echo "🏠 Building for localnet (4M1VdxXK36E1TNWuP86qydZYKWggFa6hgdZhc4VTUsUv)"
    anchor build
    ;;
  *)
    echo "❌ Unknown network: $NETWORK"
    echo "Usage: $0 [mainnet-beta|devnet|staging|localnet]"
    exit 1
    ;;
esac

echo "✅ Build completed for $NETWORK"

# Extract program ID from the built program
PROGRAM_SO="target/deploy/saguaro_gatekeeper.so"
if [ -f "$PROGRAM_SO" ]; then
  echo "📦 Program binary: $PROGRAM_SO"
  echo "📏 Size: $(ls -lh $PROGRAM_SO | awk '{print $5}')"
fi