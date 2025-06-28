use anchor_lang::prelude::*;
use crate::{ValidateSandwichValidators, GatekeeperError, SandwichValidators};
use crate::constants::SLOTS_PER_EPOCH;

/// Handles the `validate_sandwich_validators` instruction with minimal compute usage.
///
/// This function checks if the current slot is gated by reading only the specific bit
/// from the bitmap with optimized validation relying on reliable Clock sysvar.
///
/// # CPI Safety Model
/// This instruction is designed for safe Cross-Program Invocation (CPI) by third-party programs:
/// - **Fail-Open Design**: If no PDA exists for current epoch, validation passes (allows operation)
/// - **Authority-Independent**: CPI callers only need to provide multisig_authority pubkey (no signer required)
/// - **Transaction Atomicity**: SlotIsGated error will cause entire transaction to fail, providing sandwich protection
/// - **No State Changes**: This is a read-only validation function that never modifies blockchain state
///
/// # Security Notes
/// - Validates PDA address to prevent bypass attacks using current epoch from Clock sysvar
/// - PDA derivation failure results in fail-open behavior (ungated) 
/// - Uses safe error handling to prevent panics during CPI calls
/// - Clock sysvar provides reliable current slot/epoch data
///
/// # Arguments
/// * `ctx` - The context containing `ValidateSandwichValidators` accounts
///
/// # Behavior
/// 1. Derives expected PDA address using current epoch from Clock sysvar
/// 2. If PDA doesn't match or doesn't exist, treats slot as ungated (fail-open)
/// 3. If PDA exists and matches, checks specific bit in bitmap for current slot
/// 4. Returns SlotIsGated error only if slot is explicitly gated in bitmap
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
    // Layout: discriminator(8) + epoch(2) + bump(1) + padding(5) + bitmap
    const BITMAP_OFFSET: usize = 16; // SandwichValidators::DATA_OFFSET
    let target_pos = BITMAP_OFFSET + byte_index;

    // Only check if we can read the byte (bounds safety)
    if let Some(&byte) = data.get(target_pos) {
        if (byte >> bit_index) & 1 == 1 {
            return err!(GatekeeperError::SlotIsGated);
        }
    }

    Ok(())
}