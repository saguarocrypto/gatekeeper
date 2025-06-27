#!/bin/bash

# Update program ID in lib.rs and Anchor.toml

if [ -z "$1" ]; then
    echo "Usage: $0 <PROGRAM_ID>"
    exit 1
fi

PROGRAM_ID=$1
PROGRAM_NAME="saguaro_gatekeeper"
PROGRAM_DIR="saguaro-gatekeeper"

echo "Updating program ID to: $PROGRAM_ID"

# Update lib.rs
LIB_RS_PATH="programs/${PROGRAM_DIR}/src/lib.rs"
if [[ "$OSTYPE" == "darwin"* ]]; then
    sed -i '' "s/declare_id!(\".*\");/declare_id!(\"$PROGRAM_ID\");/" "$LIB_RS_PATH"
else
    sed -i "s/declare_id!(\".*\");/declare_id!(\"$PROGRAM_ID\");/" "$LIB_RS_PATH"
fi

# Update Anchor.toml
if [[ "$OSTYPE" == "darwin"* ]]; then
    sed -i '' "s/^${PROGRAM_NAME} = \".*\"$/${PROGRAM_NAME} = \"$PROGRAM_ID\"/" Anchor.toml
else
    sed -i "s/^${PROGRAM_NAME} = \".*\"$/${PROGRAM_NAME} = \"$PROGRAM_ID\"/" Anchor.toml
fi

echo "✓ Updated lib.rs and Anchor.toml"
echo "Now run: anchor build"