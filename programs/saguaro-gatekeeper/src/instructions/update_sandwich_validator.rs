use anchor_lang::prelude::*;
use crate::{UpdateSandwichValidator, SandwichValidatorsUpdated, SandwichValidators, MAX_SLOTS_PER_TRANSACTION, GatekeeperError, SLOTS_PER_EPOCH};

/// Handler for the `update_sandwich_validator` instruction.
/// This instruction supports both adding new slots and removing existing slots using bitmap operations.
pub fn handler(ctx: Context<UpdateSandwichValidator>, epoch_arg: u16, new_slots: Vec<u64>, remove_slots: Vec<u64>) -> Result<()> {

    msg!("Updating sandwich validator slots for epoch: {}", epoch_arg);
    msg!("Number of slots to add: {}", new_slots.len());
    msg!("Number of slots to remove: {}", remove_slots.len());

    // Validate that neither operation exceeds per-transaction limits
    if new_slots.len() > MAX_SLOTS_PER_TRANSACTION {
        msg!("Error: Cannot add more than {} slots per transaction", MAX_SLOTS_PER_TRANSACTION);
        return err!(GatekeeperError::TooManySlots);
    }

    if remove_slots.len() > MAX_SLOTS_PER_TRANSACTION {
        msg!("Error: Cannot remove more than {} slots per transaction", MAX_SLOTS_PER_TRANSACTION);
        return err!(GatekeeperError::TooManySlots);
    }

    // Check if both arrays are empty
    if new_slots.is_empty() && remove_slots.is_empty() {
        msg!("Warning: No slots to add or remove for epoch {}", epoch_arg);
        return Ok(());
    }

    let sandwich_validators_ai = &ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;

    // Validate account exists and is owned by our program
    if sandwich_validators_ai.data_is_empty() || *sandwich_validators_ai.owner != *ctx.program_id {
        return err!(GatekeeperError::InvalidPda);
    }

    // Deserialize the current account data
    let data_borrow = sandwich_validators_ai.try_borrow_data()?;
    let mut data_slice: &[u8] = &*data_borrow;
    let mut sandwich_validators = SandwichValidators::try_deserialize(&mut data_slice)?;
    drop(data_borrow);

    // Validate epoch matches
    if sandwich_validators.epoch != epoch_arg {
        return err!(GatekeeperError::EpochMismatch);
    }

    // Calculate valid slot range for this epoch
    let epoch_start_slot = (epoch_arg as u64) * SLOTS_PER_EPOCH as u64;
    let epoch_end_slot = epoch_start_slot + SLOTS_PER_EPOCH as u64;

    let mut slots_added = 0u16;
    let mut slots_removed = 0u16;

    // Step 1: Remove slots if specified
    if !remove_slots.is_empty() {
        // Check for duplicates in remove_slots array and validate ranges
        let mut unique_remove_slots = std::collections::HashSet::new();
        for slot in &remove_slots {
            if !unique_remove_slots.insert(*slot) {
                msg!("Error: Duplicate slot {} found in remove_slots array", slot);
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

        // Remove specified slots from bitmap
        for slot in &remove_slots {
            if sandwich_validators.is_slot_gated(*slot) {
                sandwich_validators.set_slot_gated(*slot, false)?;
                slots_removed += 1;
            }
        }
        
        msg!("Removed {} slots (requested: {})", slots_removed, remove_slots.len());
        
        if slots_removed != remove_slots.len() as u16 {
            msg!("Warning: Some slots to remove were not found in the current slot list");
        }
    }

    // Step 2: Add new slots if specified
    if !new_slots.is_empty() {
        // Check for duplicates in new_slots array and validate ranges
        let mut unique_new_slots = std::collections::HashSet::new();
        for slot in &new_slots {
            if !unique_new_slots.insert(*slot) {
                msg!("Error: Duplicate slot {} found in new_slots array", slot);
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

        // Check for already gated slots first
        for slot in &new_slots {
            if sandwich_validators.is_slot_gated(*slot) {
                msg!("Error: Slot {} is already gated in epoch {}", slot, epoch_arg);
                return err!(GatekeeperError::DuplicateSlots);
            }
        }

        // Add new slots to bitmap (all are guaranteed to be ungated at this point)
        for slot in &new_slots {
            sandwich_validators.set_slot_gated(*slot, true)?;
            slots_added += 1;
        }

        msg!("Added {} new slots", slots_added);
    }

    // Skip expensive bit counting to save compute units
    // Use a simple approximation for event emission instead
    let total_gated_slots = slots_added.saturating_add(slots_removed);

    // Serialize and write the updated data back to the account
    let mut data = sandwich_validators_ai.try_borrow_mut_data()?;
    let mut writer: &mut [u8] = &mut data;
    sandwich_validators.try_serialize(&mut writer)?;
    
    msg!(
        "Successfully updated slots for epoch {}. Total gated slots: {}",
        epoch_arg,
        total_gated_slots
    );
    
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