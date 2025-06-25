use anchor_lang::prelude::*;
use crate::{SetPauseState, PauseStateChanged};

/// Handler for the `set_pause_state` instruction.
/// 
/// Allows the multisig authority to pause or unpause the program's administrative operations.
/// When paused, set, update, and close operations will fail gracefully with ProgramPaused error.
/// The validate instruction remains unaffected to maintain CPI compatibility.
/// 
/// # Security Features:
/// - Requires multisig authority as signer and authority verification
/// - Updates pause state atomically
/// - Logs pause state changes for transparency
/// - Fails gracefully for CPI callers when operations are paused
pub fn handler(ctx: Context<SetPauseState>, is_paused: bool) -> Result<()> {
    let pause_state = &mut ctx.accounts.pause_state;
    let multisig_authority = &ctx.accounts.multisig_authority;

    // Update the pause state
    let previous_state = pause_state.is_paused;
    pause_state.is_paused = is_paused;

    // Log the state change for transparency
    msg!(
        "Pause state changed: {} -> {}",
        previous_state,
        is_paused
    );

    // Emit event for monitoring
    emit!(PauseStateChanged {
        authority: *multisig_authority.key,
        is_paused,
    });

    Ok(())
}
