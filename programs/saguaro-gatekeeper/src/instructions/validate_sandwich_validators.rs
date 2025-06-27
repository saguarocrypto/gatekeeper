use anchor_lang::prelude::*;
use crate::{ValidateSandwichValidators, GatekeeperError, SLOTS_PER_EPOCH};

/// Handles the `validate_sandwich_validators` instruction with minimal compute usage.
///
/// This function checks if the current slot is gated by reading only the specific bit
/// from the bitmap without any unnecessary validation or deserialization.
///
/// # Arguments
/// * `ctx` - The context containing `ValidateSandwichValidators` accounts
///
/// # Behavior
/// 1. Gets current slot from Clock sysvar
/// 2. Calculates the exact byte and bit position for the slot
/// 3. Directly reads the target byte from the account data
/// 4. Returns SlotIsGated error if the bit is set, otherwise passes
pub fn handler(ctx: Context<ValidateSandwichValidators>) -> Result<()> {
    let clock = &ctx.accounts.clock;
    let current_slot = clock.slot;
    let current_epoch = clock.epoch as u16;

    let pda_account = &ctx.accounts.sandwich_validators;



    // Minimal deserialization to check epoch
    let data = pda_account.try_borrow_data()?;
    if data.len() < 10 {
        return Ok(()); // Invalid data, treat as ungated
    }

    let pda_epoch = u16::from_le_bytes(data[8..10].try_into().unwrap());
    if pda_epoch != current_epoch {
        return Ok(()); // PDA for wrong epoch, treat as ungated
    }

    // Direct slot offset calculation (avoid epoch conversion)
    let slot_offset = (current_slot % SLOTS_PER_EPOCH as u64) as usize;
    let byte_index = slot_offset >> 3;  // Bit shift instead of division
    let bit_index = slot_offset & 7;    // Bit mask instead of modulo

    // Direct byte access at calculated position
    // Layout: discriminator(8) + epoch(2) + vec_len(4) + bitmap
    const BITMAP_OFFSET: usize = 14;
    let target_pos = BITMAP_OFFSET + byte_index;

    // Only check if we can read the byte
    if let Some(&byte) = data.get(target_pos) {
        if (byte >> bit_index) & 1 == 1 {
            return err!(GatekeeperError::SlotIsGated);
        }
    }

    Ok(())
}