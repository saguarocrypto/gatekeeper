use anchor_lang::prelude::*;
use crate::{PauseState, GatekeeperError};

/// Utility function to check if the program is paused.
/// 
/// This function attempts to load the pause state PDA and check if the program is paused.
/// If the pause state PDA doesn't exist, the program is considered unpaused (allows normal operation).
/// 
/// # Parameters:
/// - `pause_state_info`: AccountInfo for the pause state PDA
/// 
/// # Returns:
/// - `Ok(())` if the program is not paused or pause state doesn't exist
/// - `Err(GatekeeperError::ProgramPaused)` if the program is paused
/// 
/// # Security Features:
/// - Graceful handling of non-existent pause state (treats as unpaused)
/// - Clear error messaging for CPI callers
/// - Fails fast to minimize compute usage when paused
pub fn check_not_paused(pause_state_info: &AccountInfo) -> Result<()> {
    // If the pause state account doesn't exist, consider the program unpaused
    if pause_state_info.data_is_empty() || pause_state_info.owner != &crate::ID {
        return Ok(());
    }

    // Try to deserialize the pause state
    match PauseState::try_deserialize(&mut &pause_state_info.data.borrow()[..]) {
        Ok(pause_state) => {
            if pause_state.is_paused {
                msg!("🛑 Operation blocked: Program is currently paused");
                return err!(GatekeeperError::ProgramPaused);
            }
            Ok(())
        }
        Err(_) => {
            // If we can't deserialize, treat as unpaused to be safe
            msg!("Warning: Could not deserialize pause state, treating as unpaused");
            Ok(())
        }
    }
}

/// Derives the pause state PDA address.
/// 
/// This function derives the canonical pause state PDA address using the "pause_state" seed.
/// 
/// # Returns:
/// - `(Pubkey, u8)`: The PDA address and canonical bump
pub fn get_pause_state_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"pause_state"], &crate::ID)
}
