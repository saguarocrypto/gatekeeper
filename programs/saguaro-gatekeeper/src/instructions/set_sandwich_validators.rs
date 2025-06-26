use anchor_lang::prelude::*;
use crate::{GatekeeperError, SetSandwichValidators, SandwichValidatorsSet, SandwichValidators, MAX_SLOTS_PER_TRANSACTION, SLOTS_PER_EPOCH, INITIAL_BITMAP_SIZE_BYTES, SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE};

/// Handler for the `set_sandwich_validators` instruction.
pub fn handler(ctx: Context<SetSandwichValidators>, epoch_arg: u16, slots_arg: Vec<u64>) -> Result<()> {

    msg!("Setting sandwich validators for epoch: {}", epoch_arg);
    msg!("Number of slots: {}", slots_arg.len());

    // Validate the slots array length to prevent excessive data usage per transaction
    if slots_arg.len() > MAX_SLOTS_PER_TRANSACTION {
        return err!(GatekeeperError::TooManySlots);
    }

    // Check for empty slot array (issue warning but allow)
    if slots_arg.is_empty() {
        msg!("Warning: Setting empty slot list for epoch {}", epoch_arg);
    }

    // Calculate valid slot range for this epoch
    let epoch_start_slot = (epoch_arg as u64) * SLOTS_PER_EPOCH as u64;
    let epoch_end_slot = epoch_start_slot + SLOTS_PER_EPOCH as u64;

    // Check for duplicate slots and validate slot ranges
    let mut unique_slots = std::collections::HashSet::new();
    for slot in &slots_arg {
        if !unique_slots.insert(*slot) {
            return err!(GatekeeperError::DuplicateSlots);
        }

        // Validate slot is within the epoch boundaries
        if *slot < epoch_start_slot || *slot >= epoch_end_slot {
            msg!(
                "Error: Slot {} is outside epoch {} boundaries [{}, {})",
                slot, epoch_arg, epoch_start_slot, epoch_end_slot
            );
            return err!(GatekeeperError::SlotOutOfRange);
        }
    }

    let sandwich_validators_ai = &ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;
    let system_program = &ctx.accounts.system_program;

    // Check if account needs to be created
    if sandwich_validators_ai.data_is_empty() {
        // Create the account with the full size (now within 10KB limit)
        let rent = Rent::get()?;
        let account_size = SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE + INITIAL_BITMAP_SIZE_BYTES;
        let lamports = rent.minimum_balance(account_size);
        
        // Create account using direct invoke_signed
        let create_account_ix = anchor_lang::solana_program::system_instruction::create_account(
            multisig_authority.key,
            sandwich_validators_ai.key,
            lamports,
            account_size as u64,
            ctx.program_id,
        );
        
        anchor_lang::solana_program::program::invoke_signed(
            &create_account_ix,
            &[
                multisig_authority.to_account_info(),
                sandwich_validators_ai.to_account_info(),
                system_program.to_account_info(),
            ],
            &[&[
                b"sandwich_validators",
                multisig_authority.key().as_ref(),
                &epoch_arg.to_le_bytes(),
                &[ctx.bumps.sandwich_validators],
            ]],
        )?;
        
        msg!("Successfully created account with size: {} bytes", account_size);
    } else {
        // Account exists, verify it's owned by our program
        if sandwich_validators_ai.owner != ctx.program_id {
            return err!(GatekeeperError::InvalidPda);
        }
    }

    // Create and serialize the SandwichValidators data
    let mut sandwich_validators = SandwichValidators {
        epoch: epoch_arg,
        slots: vec![0u8; INITIAL_BITMAP_SIZE_BYTES], // Initialize bitmap with all slots ungated
        bump: ctx.bumps.sandwich_validators,
    };

    // Set the specified slots as gated in the bitmap
    for slot in &slots_arg {
        sandwich_validators.set_slot_gated(*slot, true)?;
    }

    // Serialize and write the data to the account
    let mut data = sandwich_validators_ai.try_borrow_mut_data()?;
    let mut writer: &mut [u8] = &mut data;
    sandwich_validators.try_serialize(&mut writer)?;
    
    msg!("Successfully set sandwich validators for epoch {} with {} gated slots", epoch_arg, slots_arg.len());
    
    // Emit event for monitoring
    emit!(SandwichValidatorsSet {
        authority: *multisig_authority.key,
        epoch: epoch_arg,
        slot_count: slots_arg.len() as u16,
    });
    
    Ok(())
}