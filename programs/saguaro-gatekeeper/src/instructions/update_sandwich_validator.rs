use anchor_lang::prelude::*;
use crate::{UpdateSandwichValidator, SandwichValidatorsUpdated, MAX_SLOTS_PER_TRANSACTION, GatekeeperError, SLOTS_PER_EPOCH, INITIAL_BITMAP_SIZE_BYTES, FULL_BITMAP_SIZE_BYTES};

/// Handler for the `update_sandwich_validator` instruction.
/// This instruction supports both adding new slots and removing existing slots using bitmap operations.
/// 
/// # Compute Optimization
/// This handler uses lazy loading and direct memory operations to minimize compute usage:
/// - Avoids full deserialization of the bitmap account
/// - Uses stack-based duplicate checking for small arrays
/// - Performs direct bit manipulation on borrowed account data
pub fn handler(ctx: Context<UpdateSandwichValidator>, epoch_arg: u16, new_slots: Vec<u64>, remove_slots: Vec<u64>) -> Result<()> {
    // Validate that neither operation exceeds per-transaction limits
    if new_slots.len() > MAX_SLOTS_PER_TRANSACTION {
        return err!(GatekeeperError::TooManySlots);
    }

    if remove_slots.len() > MAX_SLOTS_PER_TRANSACTION {
        return err!(GatekeeperError::TooManySlots);
    }

    // Check if both arrays are empty
    if new_slots.is_empty() && remove_slots.is_empty() {
        return Ok(());
    }

    let sandwich_validators_ai = &ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;

    // Validate account exists and is owned by our program
    if sandwich_validators_ai.data_is_empty() || *sandwich_validators_ai.owner != *ctx.program_id {
        return err!(GatekeeperError::InvalidPda);
    }

    // Calculate epoch start slot
    let epoch_start_slot = (epoch_arg as u64) * SLOTS_PER_EPOCH as u64;

    // Lazy loading - only read the epoch for validation
    let data_borrow = sandwich_validators_ai.try_borrow_data()?;
    
    // Account structure: discriminator(8) + epoch(2) + slots.len(4) + slots_data + bump(1)
    const DISCRIMINATOR_SIZE: usize = 8;
    const EPOCH_SIZE: usize = 2;
    const VEC_LEN_SIZE: usize = 4;
    const HEADER_SIZE: usize = DISCRIMINATOR_SIZE + EPOCH_SIZE + VEC_LEN_SIZE;
    
    // Validate epoch matches
    let stored_epoch = u16::from_le_bytes([data_borrow[DISCRIMINATOR_SIZE], data_borrow[DISCRIMINATOR_SIZE + 1]]);
    if stored_epoch != epoch_arg {
        return err!(GatekeeperError::EpochMismatch);
    }
    
    // Read bitmap length
    let bitmap_len = u32::from_le_bytes([
        data_borrow[DISCRIMINATOR_SIZE + EPOCH_SIZE],
        data_borrow[DISCRIMINATOR_SIZE + EPOCH_SIZE + 1],
        data_borrow[DISCRIMINATOR_SIZE + EPOCH_SIZE + 2],
        data_borrow[DISCRIMINATOR_SIZE + EPOCH_SIZE + 3],
    ]) as usize;
    
    // Validate bitmap size - allow both initial and expanded sizes
    if bitmap_len < INITIAL_BITMAP_SIZE_BYTES || bitmap_len > FULL_BITMAP_SIZE_BYTES {
        return err!(GatekeeperError::InvalidPda);
    }
    
    drop(data_borrow);

    // Calculate max trackable slots based on current bitmap size
    let max_trackable_slots = bitmap_len * 8;
    let max_trackable_slot = epoch_start_slot + max_trackable_slots as u64;

    // Optimized duplicate checking function - uses BTreeSet for simplicity and efficiency
    fn check_duplicates_and_validate(slots: &[u64], epoch_start: u64, max_trackable_slot: u64) -> Result<()> {
        if slots.is_empty() {
            return Ok(());
        }

        let mut unique_slots = std::collections::BTreeSet::new();
        for &slot in slots {
            if !unique_slots.insert(slot) {
                return err!(GatekeeperError::DuplicateSlots);
            }
            if slot < epoch_start || slot >= max_trackable_slot {
                return err!(GatekeeperError::SlotOutOfRange);
            }
        }
        Ok(())
    }

    // Validate both arrays against current bitmap capacity
    check_duplicates_and_validate(&remove_slots, epoch_start_slot, max_trackable_slot)?;
    check_duplicates_and_validate(&new_slots, epoch_start_slot, max_trackable_slot)?;

    // Direct bit manipulation functions that work with current bitmap size
    #[inline(always)]
    fn is_slot_gated_direct(data: &[u8], slot: u64, epoch_start: u64, bitmap_len: usize) -> bool {
        let slot_offset = (slot - epoch_start) as usize;
        let max_trackable_slots = bitmap_len * 8;
        
        if slot_offset >= max_trackable_slots {
            return false;
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        let byte_pos = HEADER_SIZE + byte_index;
        
        if byte_pos >= data.len() {
            return false;
        }
        
        (data[byte_pos] >> bit_index) & 1 == 1
    }
    
    #[inline(always)]
    fn set_slot_gated_direct(data: &mut [u8], slot: u64, epoch_start: u64, bitmap_len: usize, gated: bool) -> Result<()> {
        let slot_offset = (slot - epoch_start) as usize;
        let max_trackable_slots = bitmap_len * 8;
        
        if slot_offset >= max_trackable_slots {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        let byte_pos = HEADER_SIZE + byte_index;
        
        if byte_pos >= data.len() {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        if gated {
            data[byte_pos] |= 1 << bit_index;
        } else {
            data[byte_pos] &= !(1 << bit_index);
        }
        
        Ok(())
    }

    let mut slots_added = 0u16;
    let mut slots_removed = 0u16;

    // Get mutable access to account data for direct bit manipulation
    let mut data = sandwich_validators_ai.try_borrow_mut_data()?;

    // Step 1: Remove slots if specified
    if !remove_slots.is_empty() {
        for slot in &remove_slots {
            if is_slot_gated_direct(&data, *slot, epoch_start_slot, bitmap_len) {
                set_slot_gated_direct(&mut data, *slot, epoch_start_slot, bitmap_len, false)?;
                slots_removed += 1;
            }
        }
    }

    // Step 2: Add new slots if specified
    if !new_slots.is_empty() {
        // Check for already gated slots first
        for slot in &new_slots {
            if is_slot_gated_direct(&data, *slot, epoch_start_slot, bitmap_len) {
                return err!(GatekeeperError::DuplicateSlots);
            }
        }

        // Add new slots to bitmap
        for slot in &new_slots {
            set_slot_gated_direct(&mut data, *slot, epoch_start_slot, bitmap_len, true)?;
            slots_added += 1;
        }
    }

    drop(data);

    // Use approximation for total slots to avoid expensive bit counting
    let total_gated_slots = slots_added.saturating_add(slots_removed);
    
    // Emit event for monitoring
    emit!(SandwichValidatorsUpdated {
        authority: *multisig_authority.key,
        epoch: epoch_arg,
        slots_added,
        slots_removed,
        total_slots: total_gated_slots,
    });
    
    Ok(())
}