# Saguaro Gatekeeper

## Overview

Gatekeeper is a Solana program for on-chain permissions. 

This version of Gatekeeper was built to prevent toxic MEV in the Solana ecosystem, particularly sandwich attacks. 

### Functionality 

Gatekeeper integrates with existing Solana programs. This program:
1. Uses [sandwiched.me](https://sandwiched.me/sandwiches) to determine the top validators that support sandwich attacks.
2. Calculates when those sandwiching validators are in the leader slot.
3. Causes an associated transaction to fail when one of the sandwiching validators is in the leader slot.

The user will then need to re-submit the transaction. The transaction succeeds when a non-sandwiching validator is in the leader slot.

For detailed information about all available instructions, including core CRUD operations and helper functions, see the [Saguaro Gatekeeper Instructions Guide](programs/saguaro-gatekeeper/src/README.md).

## Deployment

Gatekeeper is deployed in the following Program ID:

- Mainnet: `saGUaroo4mjAcckhEPhtSRthGgFLdQpBvQvuwdf7YG3`

## CPI Integration Guide

Third-party Solana programs integrate with Gatekeeper via Cross-Program Invocation (CPI).

### Design

To integrate with Gatekeeper, your program will call the `validate_sandwich_validators` instruction. This instruction will either succeed and allow the transaction to continue or fail (blocking the transaction) if the current slot is "gated" by Gatekeeper's configuration.

This mechanism uses a "fail-open" design:

- If the current slot **is gated**, the instruction fails, causing the parent transaction to fail.
- If the current slot **is not gated**, the instruction succeeds.
- If the Gatekeeper is **not configured** for the current epoch (i.e., the PDA account doesn't exist), the instruction succeeds.

### Deriving the PDA Address

To integrate with Gatekeeper, you must use the `multisig_authority` public key controlling the Saguaro Gatekeeper configuration. This key is essential for deriving the correct PDA address.

- Saguaro's `multisig_authority` public key: `GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp`

The `sandwich_validators` account is a Program-Derived Address (PDA). Your instruction must derive this address to include it in the CPI call. The seeds for the PDA are:

- `b"sandwich_validators"`
- The `multisig_authority` public key
- The current `epoch`, encoded as `u16` little-endian bytes

Here is a Rust example of how to derive the PDA within your instruction handler:

```rust
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    clock::Clock,
    program::invoke,
    instruction::Instruction,
    sysvar::Sysvar,
};

// The public key of the Saguaro Gatekeeper program
const GATEKEEPER_PROGRAM_ID: Pubkey = pubkey!("saGUaroo4mjAcckhEPhtSRthGgFLdQpBvQvuwdf7YG3");
// The public key of the multisig authority for the gatekeeper configuration
const GATEKEEPER_AUTHORITY: Pubkey = pubkey!("GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp");

// Inside your instruction handler...
let clock = Clock::get()?;
let current_epoch = clock.epoch as u16;

let (pda_address, _bump_seed) = Pubkey::find_program_address(
    &[
        b"sandwich_validators",
        &GATEKEEPER_AUTHORITY.to_bytes(),
        &current_epoch.to_le_bytes(),
    ],
    &GATEKEEPER_PROGRAM_ID,
);

```

### Building and Invoking the CPI

Once you have the PDA address, you can build and invoke the `validate_sandwich_validators` instruction using the Anchor CPI module.

The instruction requires the following accounts:

1. `sandwich_validators`: The PDA you derived.
2. `multisig_authority`: Gatekeeper's authority key.
3. `clock`: The Clock sysvar.

### Adding Gatekeeper as a Dependency

First, add the Saguaro Gatekeeper program to your `Cargo.toml`:

```toml
[dependencies]
saguaro-gatekeeper = { git = "https://github.com/saguarocrypto/gatekeeper", features = ["cpi"] }
anchor-lang = "0.30.0"
```

### Modern Anchor CPI Implementation

```rust
use anchor_lang::prelude::*;
use saguaro_gatekeeper::{
    program::SaguaroGatekeeper,
    accounts::ValidateSandwichValidators,
    cpi,
};

// Your instruction that needs sandwich protection
#[derive(Accounts)]
pub struct YourProtectedInstruction<'info> {
    // Your regular accounts...
    pub user: Signer<'info>,
    
    // Required accounts for sandwich validation CPI
    /// CHECK: PDA will be validated by the gatekeeper program
    pub sandwich_validators: AccountInfo<'info>,
    /// CHECK: This is the multisig authority pubkey (GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp)
    pub multisig_authority: AccountInfo<'info>,
    pub clock: Sysvar<'info, Clock>,
    pub gatekeeper_program: Program<'info, SaguaroGatekeeper>,
}

pub fn your_protected_instruction(ctx: Context<YourProtectedInstruction>) -> Result<()> {
    // First, validate that the current slot is not gated
    let cpi_accounts = ValidateSandwichValidators {
        sandwich_validators: ctx.accounts.sandwich_validators.to_account_info(),
        multisig_authority: ctx.accounts.multisig_authority.to_account_info(),
        clock: ctx.accounts.clock.to_account_info(),
    };

    let cpi_ctx = CpiContext::new(
        ctx.accounts.gatekeeper_program.to_account_info(),
        cpi_accounts,
    );

    // This will return an error if the current slot is gated
    cpi::validate_sandwich_validators(cpi_ctx)?;

    // Your actual instruction logic here
    // This will only execute if the slot is NOT gated
    msg!("Slot is not gated, proceeding with instruction");

    Ok(())
}
```

### Client-Side Account Setup

When calling your instruction from the client, you need to provide the correct accounts:

```typescript
// TypeScript/JavaScript client code
import { PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY } from '@solana/web3.js';

const SAGUARO_GATEKEEPER_PROGRAM_ID = new PublicKey('saGUaroo4mjAcckhEPhtSRthGgFLdQpBvQvuwdf7YG3');
const MULTISIG_AUTHORITY = new PublicKey('GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp');

// Get current epoch to derive the PDA
const epochInfo = await connection.getEpochInfo();
const currentEpoch = epochInfo.epoch;

// Derive the sandwich validators PDA
const [sandwichValidatorsPda] = PublicKey.findProgramAddressSync(
  [
    Buffer.from('sandwich_validators'),
    MULTISIG_AUTHORITY.toBuffer(),
    Buffer.from(new Uint16Array([currentEpoch]).buffer)
  ],
  SAGUARO_GATEKEEPER_PROGRAM_ID
);

console.log(`Expected PDA: ${sandwichValidatorsPda.toBase58()}`);
// Example for epoch 816: 9cXFYv4TJp95wjWtBennqivmR3kBhVYPfmok7SyAaLwu

// Create your instruction
const instruction = await yourProgram.methods
  .yourProtectedInstruction()
  .accounts({
    user: userKeypair.publicKey,
    sandwichValidators: sandwichValidatorsPda,
    multisigAuthority: MULTISIG_AUTHORITY,
    clock: SYSVAR_CLOCK_PUBKEY,
    gatekeeperProgram: SAGUARO_GATEKEEPER_PROGRAM_ID,
    // ... your other accounts
  })
  .instruction();
```

### Error Handling

The CPI will fail with error code `6005` (SlotIsGated) if the current slot is gated:

```rust
use saguaro_gatekeeper::error::GatekeeperError;

// In your error handling
match error {
    GatekeeperError::SlotIsGated => {
        msg!("Transaction blocked: Current slot is gated for sandwich protection");
        return Err(error.into());
    }
    _ => return Err(error.into()),
}
```

### Key Points

**Fail-Open Design**: If the PDA doesn't exist for the current epoch, the validation passes (allows the operation). This ensures your program continues working even if sandwich protection isn't set up.

**Authority Independence**: You only need to provide the multisig authority pubkey (`GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp`) - no signing required.

**Automatic Epoch Handling**: The instruction automatically uses the current epoch from the Clock sysvar to derive the correct PDA.

**Transaction Atomicity**: If the slot is gated, the entire transaction fails, providing sandwich protection.


## Gatekeeper Setup Examples

Here are examples showing how to set up and configure the Saguaro Gatekeeper using the CLI tool and TypeScript SDK:

### CLI Tool Examples

The Saguaro Sandwich Updater provides a command-line interface for managing sandwich validator configurations:

```bash
# 1. Set up validators for current epoch using a validator file
node dist/main.js \
    --rpc-url https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY \
    --authority-keypair keys/GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp.json \
    --multisig-pubkey Ga335fr6UMYqzusw6aVJp7yw81n3CG8iJuMB7a4jHSeM \
    set-validators \
    --validators-file sandwich_validators.txt \
    --epoch 816

# 2. Create account for a specific epoch
node dist/main.js \
    --rpc-url https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY \
    --authority-keypair keys/GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp.json \
    --multisig-pubkey Ga335fr6UMYqzusw6aVJp7yw81n3CG8iJuMB7a4jHSeM \
    create-account \
    --epoch 817

# 3. Modify existing configuration (gate and ungate specific slots)
node dist/main.js \
    --rpc-url https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY \
    --authority-keypair keys/GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp.json \
    --multisig-pubkey Ga335fr6UMYqzusw6aVJp7yw81n3CG8iJuMB7a4jHSeM \
    modify-validators \
    --epoch 816 \
    --gate 352033214,352033300,352033400 \
    --ungate 352033100,352033150

# 4. Validate current slot (read-only check)
node dist/main.js \
    --rpc-url https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY \
    --authority-keypair keys/GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp.json \
    validate \
    --epoch 816

# 5. Validate specific slot (read bitmap directly)
node dist/main.js \
    --rpc-url https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY \
    --authority-keypair keys/GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp.json \
    validate \
    --epoch 816 \
    --slot 352033214

# 6. Use direct execution (bypass multisig)
node dist/main.js \
    --rpc-url https://mainnet.helius-rpc.com/?api-key=YOUR_API_KEY \
    --authority-keypair keys/GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp.json \
    --direct \
    set-validators \
    --validators-file sandwich_validators.txt \
    --epoch 816
```

### TypeScript SDK Examples

```typescript
import { Connection, PublicKey, Keypair } from '@solana/web3.js';

const connection = new Connection('https://api.mainnet-beta.solana.com');
const GATEKEEPER_PROGRAM_ID = new PublicKey('saGUaroo4mjAcckhEPhtSRthGgFLdQpBvQvuwdf7YG3');
const AUTHORITY = new PublicKey('GAtE1mYyAdX7T4JEWkTEvPQoNQ6ZKCYQQzJYs6Hi8iXp');

// Example: Check if a specific slot is gated
async function checkSlotStatus(epoch: number, slot: number): Promise<boolean> {
  // Derive the PDA
  const [pdaAddress] = PublicKey.findProgramAddressSync(
    [
      Buffer.from('sandwich_validators'),
      AUTHORITY.toBuffer(),
      Buffer.from(new Uint16Array([epoch]).buffer)
    ],
    GATEKEEPER_PROGRAM_ID
  );

  try {
    const accountInfo = await connection.getAccountInfo(pdaAddress);
    if (!accountInfo) {
      console.log(`No gatekeeper configuration for epoch ${epoch} - all slots ungated`);
      return false;
    }

    // Read bitmap directly
    const HEADER_SIZE = 16;
    const bitmapData = accountInfo.data.slice(HEADER_SIZE);
    
    const epochStartSlot = epoch * 432000;
    const relativeSlot = slot - epochStartSlot;
    const byteIndex = Math.floor(relativeSlot / 8);
    const bitIndex = relativeSlot % 8;
    
    if (byteIndex >= bitmapData.length) {
      return false; // Outside bitmap capacity
    }
    
    const byte = bitmapData[byteIndex];
    const isGated = (byte >> bitIndex) & 1;
    
    console.log(`Slot ${slot} is ${isGated ? 'GATED' : 'NOT gated'}`);
    return Boolean(isGated);
    
  } catch (error) {
    console.error(`Error checking slot status: ${error}`);
    return false;
  }
}

// Example: Get all gated slots for an epoch
async function getGatedSlots(epoch: number): Promise<number[]> {
  const [pdaAddress] = PublicKey.findProgramAddressSync(
    [
      Buffer.from('sandwich_validators'),
      AUTHORITY.toBuffer(),
      Buffer.from(new Uint16Array([epoch]).buffer)
    ],
    GATEKEEPER_PROGRAM_ID
  );

  const accountInfo = await connection.getAccountInfo(pdaAddress);
  if (!accountInfo) return [];

  const gatedSlots: number[] = [];
  const HEADER_SIZE = 16;
  const bitmapData = accountInfo.data.slice(HEADER_SIZE);
  const epochStartSlot = epoch * 432000;

  for (let byteIndex = 0; byteIndex < bitmapData.length; byteIndex++) {
    const byte = bitmapData[byteIndex];
    if (byte === 0) continue;

    for (let bitIndex = 0; bitIndex < 8; bitIndex++) {
      if ((byte >> bitIndex) & 1) {
        const relativeSlot = byteIndex * 8 + bitIndex;
        const absoluteSlot = epochStartSlot + relativeSlot;
        gatedSlots.push(absoluteSlot);
      }
    }
  }

  return gatedSlots;
}

// Example usage
async function main() {
  const currentEpoch = 816;
  const testSlot = 352033214;
  
  // Check specific slot
  await checkSlotStatus(currentEpoch, testSlot);
  
  // Get all gated slots
  const gatedSlots = await getGatedSlots(currentEpoch);
  console.log(`Found ${gatedSlots.length} gated slots in epoch ${currentEpoch}`);
}
```

### Architecture Notes

The Saguaro Gatekeeper uses a clear **CRUD pattern**:

- **CREATE**: `setSandwichValidators` - Creates account (10KB initial size)
- **READ**: `validateSandwichValidators` - Validates current slot (CPI-safe)  
- **UPDATE**: `modifySandwichValidators` - Gates/ungates slots
- **DELETE**: `closeSandwichValidator` - Closes past epochs

**Key Implementation Details**:
- Accounts start at 10KB due to Solana System Program limitations
- Use `expandSandwichValidatorsBitmap` to reach full 54KB capacity (432,000 slots)
- Each bit represents one slot: `0` = ungated, `1` = gated
- PDA derivation: `[b"sandwich_validators", authority.key(), epoch.to_le_bytes()]`
- Maximum 100 slots per `modifySandwichValidators` transaction

## Support

For feedback and support requests, please submit a PR to this repo or message [@saguarocrypto](https://x.com/saguarocrypto) on Twitter/X.
