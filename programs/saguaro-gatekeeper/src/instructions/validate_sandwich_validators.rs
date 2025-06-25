use anchor_lang::prelude::*;
use crate::{ValidateSandwichValidators, GatekeeperError, SandwichValidators};

/// Handles the `validate_sandwich_validators` instruction.
///
/// This function derives the current epoch and slot from the Clock sysvar,
/// then dynamically looks up and validates the SandwichValidators PDA.
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
/// 6. If the PDA exists, deserialize it and check if the current slot is in the gated slots list
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

    // Account exists and is owned by the program, deserialize it
    let data_borrow = sandwich_validators_ai.try_borrow_data()?;
    let mut data_slice: &[u8] = &*data_borrow;
    let sandwich_validators = SandwichValidators::try_deserialize(&mut data_slice)?;

    // Check if the current slot is in the gated slots list using binary search
    // Since slots are stored in sorted order, we can use binary search for O(log n) lookup
    if sandwich_validators.slots.binary_search(&current_slot).is_ok() {
        return err!(GatekeeperError::SlotIsGated);
    }

    Ok(())
}