use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::{ExpandSandwichValidatorsBitmap, TARGET_ACCOUNT_SIZE, MAX_REALLOC_SIZE, GatekeeperError};

pub fn handler(
    ctx: Context<ExpandSandwichValidatorsBitmap>, 
    _epoch_arg: u16,
) -> Result<()> {
    let sandwich_validators_account = &ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;

    // Validate account exists and is owned by our program
    if sandwich_validators_account.data_is_empty() || *sandwich_validators_account.owner != *ctx.program_id {
        return err!(GatekeeperError::InvalidPda);
    }
    
    #[cfg(feature = "debug-logs")]
    {
        msg!("Current account size: {}", sandwich_validators_account.data_len());
        msg!("Target account size: {}", TARGET_ACCOUNT_SIZE);
    }

    // Calculate how much we need to expand with overflow protection
    let current_size = sandwich_validators_account.data_len();
    
    // Validate current size is within reasonable bounds
    if current_size > TARGET_ACCOUNT_SIZE {
        return err!(GatekeeperError::InvalidPda);
    }
    
    let bytes_needed = TARGET_ACCOUNT_SIZE.saturating_sub(current_size);
    
    if bytes_needed > 0 {
        #[cfg(feature = "debug-logs")]
        msg!("Need to expand by {} bytes", bytes_needed);
        
        // Calculate rent for the actual new size after this expansion
        let rent = Rent::get()?;
        let expansion_size = bytes_needed.min(MAX_REALLOC_SIZE);
        let new_size = current_size.checked_add(expansion_size)
            .filter(|&size| size <= TARGET_ACCOUNT_SIZE)
            .ok_or(GatekeeperError::SlotOutOfRange)?;
        
        // Only transfer rent needed for the size we're actually expanding to
        let required_lamports = rent.minimum_balance(new_size);
        let current_lamports = sandwich_validators_account.lamports();
        let additional_lamports = required_lamports.saturating_sub(current_lamports);

        if additional_lamports > 0 {
            #[cfg(feature = "debug-logs")]
            {
                msg!("Adding {} lamports for rent exemption", additional_lamports);
                msg!("Rent calculated for size: {} bytes (not full target of {} bytes)", new_size, TARGET_ACCOUNT_SIZE);
            }
            
            // Transfer additional lamports for rent exemption
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: multisig_authority.to_account_info(),
                        to: sandwich_validators_account.to_account_info(),
                    },
                ),
                additional_lamports,
            )?;
        }

        // Expand the account to the new size we calculated above
        
        #[cfg(feature = "debug-logs")]
        msg!("Expanding account by {} bytes to {} bytes", expansion_size, new_size);
        sandwich_validators_account.resize(new_size)?;
        
        // Zero out the newly allocated space
        let mut account_data = sandwich_validators_account.try_borrow_mut_data()?;
        // Use fill() which may be vectorized by the compiler
        account_data[current_size..new_size].fill(0);
        drop(account_data);
        
        #[cfg(feature = "debug-logs")]
        {
            msg!("Expansion successful, new size: {}", sandwich_validators_account.data_len());
            
            // If we haven't reached target size yet, caller needs to invoke this instruction again
            if new_size < TARGET_ACCOUNT_SIZE {
                msg!("Account needs further expansion. Current: {}, Target: {}", new_size, TARGET_ACCOUNT_SIZE);
            }
        }
    } else {
        #[cfg(feature = "debug-logs")]
        msg!("Account already at target size");
    }

    #[cfg(feature = "debug-logs")]
    {
        msg!("Expand operation completed");
        msg!("Final account size: {}", sandwich_validators_account.data_len());
    }

    Ok(())
}