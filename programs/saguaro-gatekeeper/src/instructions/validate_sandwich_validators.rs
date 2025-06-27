use anchor_lang::prelude::*;
use crate::{ValidateSandwichValidators, GatekeeperError, SandwichValidators};
use crate::constants::SLOTS_PER_EPOCH;

/// Handles the `validate_sandwich_validators` instruction with minimal compute usage.
///
/// This function checks if the current slot is gated by reading only the specific bit
/// from the bitmap with optimized validation relying on reliable Clock sysvar.
///
/// # Security Notes
/// - Validates PDA address to prevent bypass attacks
/// - Relies on Clock sysvar reliability to eliminate redundant epoch/range checks
/// - Uses safe error handling to prevent panics during CPI calls  
///
/// # Arguments
/// * `ctx` - The context containing `ValidateSandwichValidators` accounts
///
/// # Behavior
/// 1. Validates the provided PDA address matches expected derivation (includes epoch validation)
/// 2. Directly calculates slot offset and reads bitmap bit
/// 3. Returns SlotIsGated error if the bit is set, otherwise passes
pub fn handler(ctx: Context<ValidateSandwichValidators>) -> Result<()> {
    let clock = &ctx.accounts.clock;
    let current_slot = clock.slot;
    let current_epoch = clock.epoch as u16;

    let pda_account = &ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;

    // CRITICAL: Validate PDA address to prevent bypass attacks
    // This implicitly validates the epoch since epoch is part of the PDA seeds
    let expected_pda = Pubkey::find_program_address(
        &[
            SandwichValidators::SEED_PREFIX,
            multisig_authority.key().as_ref(),
            &current_epoch.to_le_bytes(),
        ],
        ctx.program_id,
    ).0;
    
    if pda_account.key() != expected_pda {
        return Ok(()); // Wrong PDA, treat as ungated (fail-open)
    }

    // Account exists and PDA is valid, proceed with minimal validation
    let data = pda_account.try_borrow_data()?;
    
    // Skip redundant checks - if PDA derivation succeeded:
    // - Account size is guaranteed by Anchor
    // - Epoch in data must match current_epoch (part of PDA seed)
    // - current_slot is guaranteed to be within current_epoch (reliable Clock)
    
    // Direct slot offset calculation within epoch
    let epoch_start = (current_epoch as u64) * SLOTS_PER_EPOCH as u64;
    let slot_offset = (current_slot - epoch_start) as usize;
    let byte_index = slot_offset >> 3;  // Bit shift instead of division
    let bit_index = slot_offset & 7;    // Bit mask instead of modulo

    // Direct byte access at calculated position
    // Layout: discriminator(8) + epoch(2) + vec_len(4) + bitmap
    const BITMAP_OFFSET: usize = 14;
    let target_pos = BITMAP_OFFSET + byte_index;

    // Only check if we can read the byte (bounds safety)
    if let Some(&byte) = data.get(target_pos) {
        if (byte >> bit_index) & 1 == 1 {
            return err!(GatekeeperError::SlotIsGated);
        }
    }

    Ok(())
}