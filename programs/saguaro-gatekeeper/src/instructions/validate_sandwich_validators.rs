use anchor_lang::prelude::*;
use crate::{ValidateSandwichValidators, GatekeeperError, SLOTS_PER_EPOCH, INITIAL_BITMAP_SIZE_BYTES};

/// Handles the `validate_sandwich_validators` instruction with lazy loading.
///
/// This function efficiently validates only the specific bit needed for the current slot
/// without deserializing the entire SandwichValidators account.
///
/// # Arguments
/// * `ctx` - The context containing `ValidateSandwichValidators` accounts
///
/// # Behavior
/// 1. Gets current epoch and slot from Clock sysvar
/// 2. Derives the expected PDA address using the multisig authority and current epoch
/// 3. Fetches the sandwich_validators account from remaining_accounts[0]
/// 4. Validates that the account key matches the expected PDA
/// 5. If the PDA doesn't exist (empty data or not owned by program), validation passes (slot is not gated)
/// 6. If the PDA exists, reads only the specific byte containing the target bit
/// 7. If the slot is gated, returns SlotIsGated error
/// 8. Otherwise, validation passes
pub fn handler(ctx: Context<ValidateSandwichValidators>) -> Result<()> {
    // Get current epoch and slot from Clock sysvar
    let clock = &ctx.accounts.clock;
    let current_epoch = clock.epoch as u16;  // Convert to u16 to match PDA derivation
    let current_slot = clock.slot;

    // Derive the expected PDA address
    let (expected_pda, _bump) = Pubkey::find_program_address(
        &[
            b"sandwich_validators",
            ctx.accounts.multisig_authority.key().as_ref(),
            &current_epoch.to_le_bytes(),
        ],
        ctx.program_id,
    );

    // Get the sandwich_validators account from remaining_accounts
    let sandwich_validators_ai = ctx.remaining_accounts
        .get(0)
        .ok_or(GatekeeperError::MissingSandwichValidatorsAccount)?;

    // Validate that the provided account matches the expected PDA
    if sandwich_validators_ai.key() != expected_pda {
        return err!(GatekeeperError::InvalidSandwichValidatorsPDA);
    }

    // Check if the account is initialized (has data and is owned by the program)
    if sandwich_validators_ai.data_is_empty() || *sandwich_validators_ai.owner != *ctx.program_id {
        // PDA does not exist or is not a valid program account for us.
        // Validation passes (slot is considered not gated).
        return Ok(());
    }

    // Calculate the slot offset within the current epoch
    let epoch_start = (current_epoch as u64) * SLOTS_PER_EPOCH as u64;
    let epoch_end = epoch_start + SLOTS_PER_EPOCH as u64;
    
    // Check if slot is within this epoch's range
    if current_slot < epoch_start || current_slot >= epoch_end {
        // Slot is not in this epoch, validation passes
        return Ok(());
    }
    
    let slot_offset = (current_slot - epoch_start) as usize;
    let max_trackable_slots = INITIAL_BITMAP_SIZE_BYTES * 8;
    
    // If slot is beyond our bitmap capacity, we assume it's not gated
    if slot_offset >= max_trackable_slots {
        return Ok(());
    }

    // Calculate which byte and bit we need to check
    let byte_index = slot_offset / 8;
    let bit_index = slot_offset % 8;
    
    // Lazy loading: Read only the specific byte we need
    let data_borrow = sandwich_validators_ai.try_borrow_data()?;
    
    // Account structure: discriminator(8) + epoch(2) + slots.len(4) + slots_data + bump(1)
    let discriminator_size = 8;
    let epoch_size = 2; 
    let vec_len_size = 4;
    let header_size = discriminator_size + epoch_size + vec_len_size;
    
    // Verify account has enough data for the header
    if data_borrow.len() < header_size {
        // Invalid account structure, validation passes
        return Ok(());
    }
    
    // Read the bitmap length from the Vec<u8> length field
    let bitmap_len = u32::from_le_bytes([
        data_borrow[discriminator_size + epoch_size],
        data_borrow[discriminator_size + epoch_size + 1],
        data_borrow[discriminator_size + epoch_size + 2],
        data_borrow[discriminator_size + epoch_size + 3],
    ]) as usize;
    
    // Verify the byte we need exists in the bitmap
    if byte_index >= bitmap_len {
        // Byte is outside the bitmap, assume slot is not gated
        return Ok(());
    }
    
    // Calculate the absolute position of the target byte
    let target_byte_pos = header_size + byte_index;
    
    // Verify account has enough data for the target byte
    if data_borrow.len() <= target_byte_pos {
        // Insufficient account data, validation passes
        return Ok(());
    }
    
    // Read only the specific byte containing our target bit
    let target_byte = data_borrow[target_byte_pos];
    
    // Check if the specific bit is set (slot is gated)
    let is_gated = (target_byte >> bit_index) & 1 == 1;
    
    if is_gated {
        return err!(GatekeeperError::SlotIsGated);
    }

    Ok(())
}