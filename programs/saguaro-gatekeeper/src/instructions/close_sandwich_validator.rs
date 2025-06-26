use anchor_lang::prelude::*;
use crate::{CloseSandwichValidator, SandwichValidatorsClosed, GatekeeperError};

/// Handles the `close_sandwich_validator` instruction.
///
/// Closes an existing `SandwichValidators` PDA for a past epoch, returning its rent
/// to the `multisig_authority`. Anchor's `close` constraint handles the actual closing
/// and rent refund automatically.
pub fn handler(ctx: Context<CloseSandwichValidator>, epoch_to_close: u16) -> Result<()> {

    let epoch = ctx.accounts.sandwich_validators.epoch;
    let authority_key = ctx.accounts.multisig_authority.key();

    // Verify the epoch_to_close matches the PDA's epoch
    if epoch != epoch_to_close {
        return err!(GatekeeperError::EpochMismatch);
    }

    // Get current epoch from clock to ensure we can only close past epochs
    let clock = Clock::get()?;
    let current_epoch = clock.epoch;
    
    // Only allow closing PDAs for past epochs (current epoch or future epochs cannot be closed)
    if u64::from(epoch_to_close) >= current_epoch {
        return err!(GatekeeperError::EpochNotFinished);
    }

    msg!("Closing SandwichValidators PDA for epoch {}", epoch);
    msg!("Current epoch: {}, closing epoch: {}", current_epoch, epoch_to_close);
    msg!("Rent will be returned to authority: {}", authority_key);
    
    // Anchor's `close` constraint in the account definition handles:
    // 1. PDA validation (seeds + bump)
    // 2. Authority verification
    // 3. Account closing and rent refund
    // No additional manual work needed.
    
    // Emit event for monitoring
    emit!(SandwichValidatorsClosed {
        authority: authority_key,
        epoch: epoch_to_close,
    });
    
    Ok(())
}