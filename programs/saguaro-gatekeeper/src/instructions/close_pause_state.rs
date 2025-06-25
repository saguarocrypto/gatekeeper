use anchor_lang::prelude::*;

use crate::{ClosePauseState, PauseStateClosed};

pub fn close_pause_state(ctx: Context<ClosePauseState>) -> Result<()> {
    let authority = ctx.accounts.multisig_authority.key();
    
    // Emit event before closing
    emit!(PauseStateClosed {
        authority: authority,
    });
    
    // The close constraint handles everything - it will:
    // 1. Transfer all lamports from pause_state to multisig_authority
    // 2. Zero out the account data
    // 3. Assign ownership back to System Program
    Ok(())
}