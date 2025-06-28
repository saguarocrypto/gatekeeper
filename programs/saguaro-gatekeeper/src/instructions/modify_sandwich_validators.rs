use anchor_lang::prelude::*;
use crate::{ModifySandwichValidators, SandwichValidatorsUpdated, MAX_SLOTS_PER_TRANSACTION, GatekeeperError, SLOTS_PER_EPOCH, INITIAL_BITMAP_SIZE_BYTES, FULL_BITMAP_SIZE_BYTES};

/// Handler for the `modify_sandwich_validators` instruction.
/// 
/// **CRUD Operation: UPDATE**
/// This instruction supports both gating and ungating slots using bitmap operations.
/// Use `expand_sandwich_validators_bitmap` first if you need more capacity.
/// 
/// # Slot Range Limits
/// - Each epoch contains exactly 432,000 slots (SLOTS_PER_EPOCH)
/// - Slots must be within the epoch range: [epoch_start, epoch_start + 432,000)
/// - Bitmap size determines how many of these 432,000 slots can be tracked
/// - Full bitmap (54,000 bytes) can track all 432,000 slots in an epoch
/// 
/// # Compute Optimization
/// This handler uses lazy loading and direct memory operations to minimize compute usage:
/// - Avoids full deserialization of the bitmap account
/// - Uses stack-based duplicate checking for small arrays
/// - Performs direct bit manipulation on borrowed account data
pub fn handler(ctx: Context<ModifySandwichValidators>, epoch_arg: u16, slots_to_gate: Vec<u64>, slots_to_ungate: Vec<u64>) -> Result<()> {
    // Compile-time assertion to ensure bitmap size is consistent with slot count
    const _: () = assert!(FULL_BITMAP_SIZE_BYTES * 8 >= SLOTS_PER_EPOCH, "Full bitmap must be able to hold all epoch slots");
    // Validate that neither operation exceeds per-transaction limits
    if slots_to_gate.len() > MAX_SLOTS_PER_TRANSACTION {
        return err!(GatekeeperError::TooManySlots);
    }

    if slots_to_ungate.len() > MAX_SLOTS_PER_TRANSACTION {
        return err!(GatekeeperError::TooManySlots);
    }

    // Check if both arrays are empty
    if slots_to_gate.is_empty() && slots_to_ungate.is_empty() {
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
    
    // SandwichValidators structure: discriminator(8) + epoch(2) + bump(1) + padding(5) + bitmap_data
    const HEADER_SIZE: usize = 16; // SandwichValidators::DATA_OFFSET
    
    // Validate epoch matches
    let stored_epoch = u16::from_le_bytes([data_borrow[8], data_borrow[9]]);
    if stored_epoch != epoch_arg {
        return err!(GatekeeperError::EpochMismatch);
    }
    
    // Calculate bitmap length from account size
    let bitmap_len = data_borrow.len() - HEADER_SIZE;
    
    // Validate bitmap size - allow both initial and expanded sizes
    // For 432,000 slots, we need exactly 54,000 bytes (432,000 bits / 8)
    if bitmap_len < INITIAL_BITMAP_SIZE_BYTES || bitmap_len > FULL_BITMAP_SIZE_BYTES {
        return err!(GatekeeperError::InvalidPda);
    }
    
    // Note: Additional bitmap size validation is handled in the max_trackable_slots calculation below
    // This allows for gradual expansion of the bitmap as needed
    
    drop(data_borrow);

    // Calculate max trackable slots based on current bitmap size with comprehensive overflow protection
    // Ensure we don't exceed the epoch boundary of 432,000 slots
    let bitmap_max_slots = bitmap_len
        .checked_mul(8)
        .filter(|&slots| slots <= usize::MAX / 2) // Additional safety margin
        .ok_or(GatekeeperError::SlotOutOfRange)?;
    let epoch_max_slots = SLOTS_PER_EPOCH;
    let max_trackable_slots = std::cmp::min(bitmap_max_slots, epoch_max_slots);
    let max_trackable_slot = epoch_start_slot
        .checked_add(max_trackable_slots as u64)
        .filter(|&slot| slot < u64::MAX / 2) // Additional safety margin
        .ok_or(GatekeeperError::SlotOutOfRange)?;

    // Optimized duplicate checking function - uses stack allocation for small arrays
    fn check_duplicates_and_validate(slots: &[u64], epoch_start: u64, max_trackable_slot: u64, epoch_end: u64) -> Result<()> {
        if slots.is_empty() {
            return Ok(());
        }

        // For small arrays (typical case), use O(n²) stack-based checking to avoid heap allocation
        // For larger arrays, fall back to BTreeSet but these are limited by MAX_SLOTS_PER_TRANSACTION
        if slots.len() <= 32 {
            // Stack-based duplicate detection for small arrays
            for (i, &slot_a) in slots.iter().enumerate() {
                // Check for duplicates in remaining elements
                for &slot_b in &slots[i + 1..] {
                    if slot_a == slot_b {
                        return err!(GatekeeperError::DuplicateSlots);
                    }
                }
                
                // Validate slot boundaries
                if slot_a < epoch_start || slot_a > epoch_end || slot_a >= max_trackable_slot {
                    return err!(GatekeeperError::SlotOutOfRange);
                }
            }
        } else {
            // Heap-based checking for larger arrays (rare case due to transaction limits)
            let mut unique_slots = std::collections::BTreeSet::new();
            for &slot in slots {
                if !unique_slots.insert(slot) {
                    return err!(GatekeeperError::DuplicateSlots);
                }
                
                if slot < epoch_start || slot > epoch_end || slot >= max_trackable_slot {
                    return err!(GatekeeperError::SlotOutOfRange);
                }
            }
        }
        Ok(())
    }

    // Calculate epoch end for validation (epoch contains exactly SLOTS_PER_EPOCH slots)
    let epoch_end_slot = epoch_start_slot.checked_add(SLOTS_PER_EPOCH as u64).ok_or(GatekeeperError::SlotOutOfRange)?;
    
    // Validate both arrays against current bitmap capacity and epoch boundaries
    check_duplicates_and_validate(&slots_to_ungate, epoch_start_slot, max_trackable_slot, epoch_end_slot - 1)?;
    check_duplicates_and_validate(&slots_to_gate, epoch_start_slot, max_trackable_slot, epoch_end_slot - 1)?;

    // Direct bit manipulation functions that work with current bitmap size and respect epoch boundaries
    #[inline(always)]
    fn is_slot_gated_direct(data: &[u8], slot: u64, epoch_start: u64, bitmap_len: usize) -> bool {
        // Validate inputs first
        if slot < epoch_start || bitmap_len == 0 {
            return false;
        }
        
        let slot_offset: usize = match (slot - epoch_start).try_into() {
            Ok(offset) => offset,
            Err(_) => return false, // Conversion failed, treat as not gated
        };
        
        // Calculate max trackable slots respecting both bitmap size and epoch boundary
        let bitmap_max_slots = match bitmap_len.checked_mul(8) {
            Some(slots) if slots <= SLOTS_PER_EPOCH => slots,
            _ => return false, // Overflow or exceeds epoch, treat as not gated
        };
        
        if slot_offset >= bitmap_max_slots {
            return false;
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        let byte_pos = match HEADER_SIZE.checked_add(byte_index) {
            Some(pos) if pos < data.len() => pos,
            _ => return false, // Overflow or out of bounds, treat as not gated
        };
        
        (data[byte_pos] >> bit_index) & 1 == 1
    }
    
    #[inline(always)]
    fn set_slot_gated_direct(data: &mut [u8], slot: u64, epoch_start: u64, bitmap_len: usize, gated: bool) -> Result<()> {
        // Validate inputs first
        if slot < epoch_start || bitmap_len == 0 {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        let slot_offset: usize = (slot - epoch_start).try_into()
            .map_err(|_| GatekeeperError::SlotOutOfRange)?;
        
        // Calculate max trackable slots respecting both bitmap size and epoch boundary
        let bitmap_max_slots = bitmap_len
            .checked_mul(8)
            .filter(|&slots| slots <= SLOTS_PER_EPOCH)
            .ok_or(GatekeeperError::SlotOutOfRange)?;
        
        if slot_offset >= bitmap_max_slots {
            return err!(GatekeeperError::SlotOutOfRange);
        }
        
        let byte_index = slot_offset / 8;
        let bit_index = slot_offset % 8;
        let byte_pos = HEADER_SIZE.checked_add(byte_index)
            .filter(|&pos| pos < data.len())
            .ok_or(GatekeeperError::SlotOutOfRange)?;
        
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

    // Step 1: Ungate slots if specified
    if !slots_to_ungate.is_empty() {
        for slot in &slots_to_ungate {
            if is_slot_gated_direct(&data, *slot, epoch_start_slot, bitmap_len) {
                set_slot_gated_direct(&mut data, *slot, epoch_start_slot, bitmap_len, false)?;
                slots_removed += 1;
            }
        }
    }

    // Step 2: Gate slots if specified
    if !slots_to_gate.is_empty() {
        // Check for already gated slots first
        for slot in &slots_to_gate {
            if is_slot_gated_direct(&data, *slot, epoch_start_slot, bitmap_len) {
                return err!(GatekeeperError::DuplicateSlots);
            }
        }

        // Gate new slots in bitmap
        for slot in &slots_to_gate {
            set_slot_gated_direct(&mut data, *slot, epoch_start_slot, bitmap_len, true)?;
            slots_added += 1;
        }
    }

    drop(data);

    // For monitoring purposes, emit the net change as an approximation
    // Note: The total_slots field represents the net change for this operation only,
    // not the actual total gated slots in the bitmap to avoid expensive scanning.
    let net_change_approximation = slots_added.saturating_sub(slots_removed);
    
    // Emit event for monitoring - only emit if there were actual changes
    if slots_added > 0 || slots_removed > 0 {
        emit!(SandwichValidatorsUpdated {
            authority: *multisig_authority.key,
            epoch: epoch_arg,
            slots_added,
            slots_removed,
            total_slots: net_change_approximation, // Net change for this operation only
        });
    }
    
    Ok(())
}