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
    
    // LargeBitmap structure: discriminator(8) + epoch(2) + bump(1) + padding(5) + bitmap_data
    const HEADER_SIZE: usize = 16; // LargeBitmap::DATA_OFFSET
    
    // Validate epoch matches
    let stored_epoch = u16::from_le_bytes([data_borrow[8], data_borrow[9]]);
    if stored_epoch != epoch_arg {
        return err!(GatekeeperError::EpochMismatch);
    }
    
    // Calculate bitmap length from account size
    let bitmap_len = data_borrow.len() - HEADER_SIZE;
    
    // Validate bitmap size - allow both initial and expanded sizes
    if bitmap_len < INITIAL_BITMAP_SIZE_BYTES || bitmap_len > FULL_BITMAP_SIZE_BYTES {
        return err!(GatekeeperError::InvalidPda);
    }
    
    drop(data_borrow);

    // Calculate max trackable slots based on current bitmap size with overflow protection
    let max_trackable_slots = bitmap_len.checked_mul(8).ok_or(GatekeeperError::SlotOutOfRange)?;
    let max_trackable_slot = epoch_start_slot.checked_add(max_trackable_slots as u64).ok_or(GatekeeperError::SlotOutOfRange)?;

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
        let max_trackable_slots = match bitmap_len.checked_mul(8) {
            Some(slots) => slots,
            None => return false, // Overflow, treat as not gated
        };
        
        if slot_offset >= max_trackable_slots {
            return false;
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        let byte_pos = match HEADER_SIZE.checked_add(byte_index) {
            Some(pos) => pos,
            None => return false, // Overflow, treat as not gated
        };
        
        if byte_pos >= data.len() {
            return false;
        }
        
        (data[byte_pos] >> bit_index) & 1 == 1
    }
    
    #[inline(always)]
    fn set_slot_gated_direct(data: &mut [u8], slot: u64, epoch_start: u64, bitmap_len: usize, gated: bool) -> Result<()> {
        let slot_offset = (slot - epoch_start) as usize;
        let max_trackable_slots = bitmap_len.checked_mul(8).ok_or(GatekeeperError::SlotOutOfRange)?;
        
        if slot_offset >= max_trackable_slots {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        let byte_pos = HEADER_SIZE.checked_add(byte_index).ok_or(GatekeeperError::SlotOutOfRange)?;
        
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

    // For monitoring purposes, emit the net change as an approximation
    // Note: The total_slots field is misleadingly named - it represents 
    // an approximation for monitoring, not the actual total gated slots.
    // Computing the actual total would require an expensive bitmap scan.
    let net_change_approximation = if slots_added >= slots_removed {
        slots_added - slots_removed
    } else {
        0 // Net decrease, show as 0 since we can't represent negative values
    };
    
    // Emit event for monitoring
    emit!(SandwichValidatorsUpdated {
        authority: *multisig_authority.key,
        epoch: epoch_arg,
        slots_added,
        slots_removed,
        total_slots: net_change_approximation, // WARNING: Not actual total, just net change for this operation
    });
    
    Ok(())
}