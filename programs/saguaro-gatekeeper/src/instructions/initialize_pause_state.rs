use anchor_lang::prelude::*;
use crate::{InitializePauseState, PauseStateInitialized};

/// Handler for the `initialize_pause_state` instruction.
/// 
/// Creates the global pause state PDA that controls emergency pause functionality.
/// This instruction can only be called once to initialize the pause state.
/// 
/// # Security Features:
/// - Requires multisig authority as signer
/// - Uses deterministic PDA with "pause_state" seed
/// - Initializes with unpaused state (is_paused = false)
/// - Stores authority and bump for future operations
pub fn handler(ctx: Context<InitializePauseState>) -> Result<()> {
    msg!(
        "Initializing pause state"
    );
    
    let pause_state = &mut ctx.accounts.pause_state;
    
    // Initialize the pause state account
    pause_state.multisig_authority = ctx.accounts.multisig_authority.key();
    pause_state.is_paused = false; // Start unpaused
    pause_state.bump = ctx.bumps.pause_state;
    
    msg!("Pause state initialized with authority: {}", pause_state.multisig_authority);
    
    // Emit event for monitoring
    emit!(PauseStateInitialized {
        authority: pause_state.multisig_authority,
    });
    
    Ok(())
}
