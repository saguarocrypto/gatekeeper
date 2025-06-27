use anchor_lang::prelude::*;
use crate::{GatekeeperError, SetSandwichValidators, SandwichValidatorsSet, MAX_SLOTS_PER_TRANSACTION, SLOTS_PER_EPOCH, INITIAL_BITMAP_SIZE_BYTES, SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE};

/// Handler for the `set_sandwich_validators` instruction.
/// 
/// # Compute Optimization
/// This handler uses direct memory operations to minimize compute usage:
/// - Optimized duplicate checking with sorting for small arrays
/// - Direct bitmap writes without full struct serialization
/// - Minimal memory allocations
pub fn handler(ctx: Context<SetSandwichValidators>, epoch_arg: u16, slots_arg: Vec<u64>) -> Result<()> {
    // Validate the slots array length to prevent excessive data usage per transaction
    if slots_arg.len() > MAX_SLOTS_PER_TRANSACTION {
        return err!(GatekeeperError::TooManySlots);
    }

    // Check for empty slot array (issue warning but allow)
    if slots_arg.is_empty() {
        #[cfg(feature = "debug-logs")]
        msg!("Warning: Setting empty slot list for epoch {}", epoch_arg);
    }

    // Calculate valid slot range for this epoch with overflow protection
    let epoch_start_slot = (epoch_arg as u64).checked_mul(SLOTS_PER_EPOCH as u64).ok_or(GatekeeperError::SlotOutOfRange)?;
    let epoch_end_slot = epoch_start_slot.checked_add(SLOTS_PER_EPOCH as u64).ok_or(GatekeeperError::SlotOutOfRange)?;

    // Optimized duplicate checking - use BTreeSet for simplicity and efficiency
    if !slots_arg.is_empty() {
        let mut unique_slots = std::collections::BTreeSet::new();
        for slot in &slots_arg {
            if !unique_slots.insert(*slot) {
                return err!(GatekeeperError::DuplicateSlots);
            }
            if *slot < epoch_start_slot || *slot >= epoch_end_slot {
                return err!(GatekeeperError::SlotOutOfRange);
            }
        }
    }

    let sandwich_validators_ai = &ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;
    let system_program = &ctx.accounts.system_program;

    // Check if account needs to be created
    if sandwich_validators_ai.data_is_empty() {
        // Create the account with the full size
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
    } else {
        // Account exists, verify it's owned by our program
        if sandwich_validators_ai.owner != ctx.program_id {
            return err!(GatekeeperError::InvalidPda);
        }
    }

    // Direct memory write optimization
    let mut data = sandwich_validators_ai.try_borrow_mut_data()?;
    
    // Account structure constants
    const DISCRIMINATOR_SIZE: usize = 8;
    const EPOCH_SIZE: usize = 2;
    const VEC_LEN_SIZE: usize = 4;
    const HEADER_SIZE: usize = DISCRIMINATOR_SIZE + EPOCH_SIZE + VEC_LEN_SIZE;
    
    // Write discriminator - use Anchor's generated discriminator for SandwichValidators
    use anchor_lang::Discriminator;
    use crate::SandwichValidators;
    data[0..8].copy_from_slice(&SandwichValidators::DISCRIMINATOR);
    
    // Write epoch
    data[8..10].copy_from_slice(&epoch_arg.to_le_bytes());
    
    // Write vector length
    data[10..14].copy_from_slice(&(INITIAL_BITMAP_SIZE_BYTES as u32).to_le_bytes());
    
    // Initialize bitmap area to zero (slots ungated by default)
    // Use fill() which may be vectorized by the compiler
    data[HEADER_SIZE..HEADER_SIZE + INITIAL_BITMAP_SIZE_BYTES].fill(0);
    
    // Set the specified slots as gated in the bitmap using direct bit manipulation
    for slot in &slots_arg {
        let slot_offset = (*slot - epoch_start_slot) as usize;
        let max_trackable_slots = INITIAL_BITMAP_SIZE_BYTES * 8;
        
        // Skip slots that are beyond the initial bitmap capacity
        // These will need to be set after the PDA is expanded
        if slot_offset >= max_trackable_slots {
            #[cfg(feature = "debug-logs")]
            msg!("Skipping slot {} (offset {}) - beyond initial bitmap capacity ({})", slot, slot_offset, max_trackable_slots);
            continue;
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        let byte_pos = HEADER_SIZE.checked_add(byte_index).ok_or(GatekeeperError::SlotOutOfRange)?;
        
        // Bounds check before array access
        if byte_pos >= data.len() {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        // Set the bit
        data[byte_pos] |= 1 << bit_index;
    }
    
    // Write bump at the end
    data[HEADER_SIZE + INITIAL_BITMAP_SIZE_BYTES] = ctx.bumps.sandwich_validators;
    
    drop(data);
    
    // Emit event for monitoring
    emit!(SandwichValidatorsSet {
        authority: *multisig_authority.key,
        epoch: epoch_arg,
        slot_count: slots_arg.len() as u16,
    });
    
    Ok(())
}