use anchor_lang::prelude::*;
use crate::{UpdateSandwichValidator, SandwichValidatorsUpdated, MAX_SLOTS_PER_TRANSACTION, MAX_SLOTS_PER_EPOCH, GatekeeperError};

/// Handler for the `update_sandwich_validator` instruction.
/// This instruction supports both adding new slots and removing existing slots.
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

    let sandwich_validators = &mut ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;

    // Get current slots
    let mut current_slots = sandwich_validators.slots.clone();
    
    // Step 1: Remove slots if specified
    if !remove_slots.is_empty() {
        // Check for duplicates in remove_slots array
        let mut unique_remove_slots = std::collections::HashSet::new();
        for slot in &remove_slots {
            if !unique_remove_slots.insert(*slot) {
                msg!("Error: Duplicate slot {} found in remove_slots array", slot);
                return err!(GatekeeperError::DuplicateSlots);
            }
        }

        // Remove specified slots from current_slots
        let initial_length = current_slots.len();
        current_slots.retain(|slot| !unique_remove_slots.contains(slot));
        let removed_count = initial_length - current_slots.len();
        
        msg!("Removed {} slots (requested: {})", removed_count, remove_slots.len());
        
        if removed_count != remove_slots.len() {
            msg!("Warning: Some slots to remove were not found in the current slot list");
        }
    }

    // Step 2: Add new slots if specified
    if !new_slots.is_empty() {
        // Calculate valid slot range for this epoch
        let epoch_start_slot = (epoch_arg as u64) * 432_000;
        let epoch_end_slot = ((epoch_arg as u64) + 1) * 432_000;

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

        // Check if any new slots already exist in current slots
        let current_slots_set: std::collections::HashSet<u64> = current_slots.iter().cloned().collect();
        for slot in &new_slots {
            if current_slots_set.contains(slot) {
                msg!("Error: Slot {} already exists in epoch {}", slot, epoch_arg);
                return err!(GatekeeperError::DuplicateSlots);
            }
        }

        // Check if adding new slots would exceed total limit
        let new_total_length = current_slots.len()
            .checked_add(new_slots.len())
            .ok_or(GatekeeperError::TooManySlots)?;
        
        if new_total_length > MAX_SLOTS_PER_EPOCH {
            msg!(
                "Error: Adding {} slots to existing {} slots would exceed maximum of {} slots per epoch",
                new_slots.len(),
                current_slots.len(),
                MAX_SLOTS_PER_EPOCH
            );
            return err!(GatekeeperError::TooManySlots);
        }

        // Add the new slots
        current_slots.extend(new_slots.clone());
        msg!("Added {} new slots", new_slots.len());
    }

    // Sort the slots for efficient binary search during validation
    current_slots.sort_unstable();

    // Calculate the new required size with overflow protection
    let slot_data_size = current_slots.len()
        .checked_mul(8)
        .ok_or(GatekeeperError::TooManySlots)?;
    let new_size = crate::SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE
        .checked_add(slot_data_size)
        .ok_or(GatekeeperError::TooManySlots)?;

    // Calculate rent for the new size
    let minimum_balance = Rent::get()?.minimum_balance(new_size);
    let current_lamports = sandwich_validators.to_account_info().lamports();

    // If more rent is needed, transfer it from the multisig authority
    if minimum_balance > current_lamports {
        let rent_diff = minimum_balance.saturating_sub(current_lamports);
        
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: multisig_authority.to_account_info(),
                    to: sandwich_validators.to_account_info(),
                },
            ),
            rent_diff,
        )?;
    }

    // Resize the account if necessary
    sandwich_validators.to_account_info().resize(new_size)?;

    // Verify account is still rent-exempt after resize
    let final_lamports = sandwich_validators.to_account_info().lamports();
    let final_rent = Rent::get()?.minimum_balance(new_size);
    if final_lamports < final_rent {
        return err!(GatekeeperError::RentNotMet);
    }

    // Update the slots array
    sandwich_validators.slots = current_slots;
    
    msg!(
        "Successfully updated slots for epoch {}. Total slots: {}",
        epoch_arg,
        sandwich_validators.slots.len()
    );
    
    // Emit event for monitoring
    emit!(SandwichValidatorsUpdated {
        authority: *multisig_authority.key,
        epoch: epoch_arg,
        slots_added: new_slots.len() as u16,
        slots_removed: remove_slots.len() as u16,
        total_slots: sandwich_validators.slots.len() as u16,
    });
    
    Ok(())
}