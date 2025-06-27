# Saguaro Gatekeeper Program


## Overview

Saguaro Gatekeeper is a Solana program designed to support on-chain permissions.

## CPI Integration Guide

This guide explains how third-party Solana programs can integrate with the Saguaro Gatekeeper via Cross-Program Invocation (CPI) to validate whether an action should be permitted in the current slot.

### Overview

The core of the integration is the `validate_sandwich_validators` instruction. Your program will call this instruction, and it will either succeed (allowing your instruction to continue) or fail (blocking the transaction) based on whether the current slot is "gated" by the gatekeeper's configuration.

This mechanism uses a "fail-open" design:
- If the current slot **is gated**, the instruction fails, causing the parent transaction to fail.
- If the current slot **is not gated**, the instruction succeeds.
- If the Gatekeeper is **not configured** for the current epoch (i.e., the PDA account doesn't exist), the instruction succeeds.

### Prerequisites

To integrate, your program needs to know the `multisig_authority` public key that controls the Saguaro Gatekeeper configuration you wish to use. This key is essential for deriving the correct PDA address.

### 1. Deriving the PDA Address

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
const GATEKEEPER_PROGRAM_ID: Pubkey = pubkey!("<GATEKEEPER_PROGRAM_ID>");
// The public key of the multisig authority for the gatekeeper configuration
const GATEKEEPER_AUTHORITY: Pubkey = pubkey!("<GATEKEEPER_MULTISIG_AUTHORITY_PUBKEY>");

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

### 2. Building and Invoking the CPI

Once you have the PDA address, you can build and invoke the `validate_sandwich_validators` instruction.

The instruction requires the following accounts:
1.  `sandwich_validators`: The PDA you derived.
2.  `multisig_authority`: The gatekeeper's authority key.
3.  `clock`: The Clock sysvar.

Here is a Rust example of the CPI call:

```rust
// Instruction data for `validate_sandwich_validators`
// The discriminator is the SHA256 hash of `global::validate_sandwich_validators`
let instruction_data: Vec<u8> = vec![227, 103, 131, 13, 162, 18, 96, 108];

let accounts = vec![
    AccountMeta::new_readonly(pda_address, false),
    AccountMeta::new_readonly(GATEKEEPER_AUTHORITY, false),
    AccountMeta::new_readonly(anchor_lang::solana_program::sysvar::clock::ID, false),
];

let cpi_instruction = Instruction {
    program_id: GATEKEEPER_PROGRAM_ID,
    accounts,
    data: instruction_data,
};

invoke(
    &cpi_instruction,
    &[
        // Pass the AccountInfo objects for the accounts required by the CPI
        // These must be passed into your instruction from the client
        ctx.accounts.sandwich_validators_pda.clone(),
        ctx.accounts.gatekeeper_authority.clone(),
        ctx.accounts.clock.clone(),
    ],
)?;
```
*Note: You must pass the `AccountInfo` for the PDA, authority, and clock into your own instruction so you can then pass them to the `invoke` function.*

### Example CPI Instruction

Here is a complete example of an Anchor instruction that integrates with the Saguaro Gatekeeper.

```rust
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::invoke,
    instruction::Instruction,
};

// Replace with the actual Gatekeeper Program ID
declare_id!("DApD5AGWB5TcN83q3NRGGdQtWa3i7w1W7rVUzsvFLBqe");

#[program]
pub mod my_program {
    use super::*;
    pub fn do_something_sensitive(ctx: Context<DoSomethingSensitive>) -> Result<()> {
        // --- Saguaro Gatekeeper CPI ---

        // 1. Define the Gatekeeper program and authority
        let gatekeeper_program_id = ctx.accounts.saguaro_gatekeeper_program.key();
        let gatekeeper_authority = ctx.accounts.gatekeeper_authority.key();

        // 2. Derive the PDA address for the current epoch
        let clock = &ctx.accounts.clock;
        let current_epoch = clock.epoch as u16;
        let (pda_address, _bump) = Pubkey::find_program_address(
            &[
                b"sandwich_validators",
                &gatekeeper_authority.to_bytes(),
                &current_epoch.to_le_bytes(),
            ],
            &gatekeeper_program_id,
        );

        // Ensure the provided PDA account matches the derived address
        require_keys_eq!(pda_address, ctx.accounts.sandwich_validators_pda.key(), "InvalidSandwichValidatorPDA");

        // 3. Build the CPI instruction
        let cpi_instruction = Instruction {
            program_id: gatekeeper_program_id,
            accounts: vec![
                AccountMeta::new_readonly(pda_address, false),
                AccountMeta::new_readonly(gatekeeper_authority, false),
                AccountMeta::new_readonly(anchor_lang::solana_program::sysvar::clock::ID, false),
            ],
            // Instruction discriminator for `validate_sandwich_validators`
            data: vec![227, 103, 131, 13, 162, 18, 96, 108],
        };

        // 4. Invoke the CPI
        invoke(
            &cpi_instruction,
            &[
                ctx.accounts.sandwich_validators_pda.to_account_info(),
                ctx.accounts.gatekeeper_authority.to_account_info(),
                ctx.accounts.clock.to_account_info(),
                ctx.accounts.saguaro_gatekeeper_program.to_account_info(),
            ],
        )?;

        // --- Gatekeeper validation passed, proceed with sensitive logic ---

        msg!("Validation successful, executing sensitive logic...");

        // ... your program's logic here ...

        Ok(())
    }
}

#[derive(Accounts)]
pub struct DoSomethingSensitive<'info> {
    // Your instruction's accounts...
    pub user: Signer<'info>,

    /// CHECK: The Saguaro Gatekeeper program address.
    pub saguaro_gatekeeper_program: AccountInfo<'info>,

    /// CHECK: The authority account for the Gatekeeper configuration.
    pub gatekeeper_authority: AccountInfo<'info>,

    /// CHECK: The PDA account for the current epoch's sandwich validators.
    pub sandwich_validators_pda: AccountInfo<'info>,

    /// The Clock sysvar, required by the Gatekeeper.
    pub clock: Sysvar<'info, Clock>,
}
```
