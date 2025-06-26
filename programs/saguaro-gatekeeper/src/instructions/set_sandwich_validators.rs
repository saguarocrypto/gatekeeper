use anchor_lang::prelude::*;
use crate::{GatekeeperError, SetSandwichValidators, SandwichValidatorsSet, MAX_SLOTS_PER_TRANSACTION};

/// Handler for the `set_sandwich_validators` instruction.
pub fn handler(ctx: Context<SetSandwichValidators>, epoch_arg: u16, slots_arg: Vec<u64>) -> Result<()> {

    msg!("Setting sandwich validators for epoch: {}", epoch_arg);
    msg!("Number of slots: {}", slots_arg.len());

    // Validate the slots array length to prevent excessive data usage per transaction
    if slots_arg.len() > MAX_SLOTS_PER_TRANSACTION {
        return err!(GatekeeperError::TooManySlots);
    }

    // Check for empty slot array (issue warning but allow)
    if slots_arg.is_empty() {
        msg!("Warning: Setting empty slot list for epoch {}", epoch_arg);
    }

    // Calculate valid slot range for this epoch
    let epoch_start_slot = (epoch_arg as u64) * 432_000;
    let epoch_end_slot = ((epoch_arg as u64) + 1) * 432_000;

    // Check for duplicate slots and validate slot ranges
    let mut unique_slots = std::collections::HashSet::new();
    for slot in &slots_arg {
        if !unique_slots.insert(*slot) {
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

    // Sort the slots for efficient binary search during validation
    let mut sorted_slots = slots_arg.clone();
    sorted_slots.sort_unstable();

    let sandwich_validators = &mut ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;

    // Set the account data (Anchor handles initialization with init_if_needed)
    sandwich_validators.epoch = epoch_arg;
    sandwich_validators.slots = sorted_slots;
    sandwich_validators.bump = ctx.bumps.sandwich_validators;
    
    msg!("Successfully set sandwich validators for epoch {} with {} slots", epoch_arg, sandwich_validators.slots.len());
    
    // Emit event for monitoring
    emit!(SandwichValidatorsSet {
        authority: *multisig_authority.key,
        epoch: epoch_arg,
        slot_count: sandwich_validators.slots.len() as u16,
    });
    
    Ok(())
}