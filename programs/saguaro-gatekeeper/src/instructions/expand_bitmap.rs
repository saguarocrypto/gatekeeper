use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::{ExpandBitmap, TARGET_ACCOUNT_SIZE, MAX_REALLOC_SIZE};

pub fn handler(
    ctx: Context<ExpandBitmap>,
) -> Result<()> {
    let large_bitmap_account = &ctx.accounts.large_bitmap;
    let multisig_authority = &ctx.accounts.multisig_authority;

    #[cfg(feature = "debug-logs")]
    {
        msg!("Current account size: {}", large_bitmap_account.data_len());
        msg!("Target account size: {}", TARGET_ACCOUNT_SIZE);
    }

    // Calculate how much we need to expand
    let current_size = large_bitmap_account.data_len();
    let bytes_needed = TARGET_ACCOUNT_SIZE.saturating_sub(current_size);
    
    if bytes_needed > 0 {
        #[cfg(feature = "debug-logs")]
        msg!("Need to expand by {} bytes", bytes_needed);
        
        // Calculate rent for target size
        let rent = Rent::get()?;
        let target_lamports = rent.minimum_balance(TARGET_ACCOUNT_SIZE);
        let current_lamports = large_bitmap_account.lamports();
        let additional_lamports = target_lamports.saturating_sub(current_lamports);

        if additional_lamports > 0 {
            #[cfg(feature = "debug-logs")]
            msg!("Adding {} lamports for rent exemption", additional_lamports);
            
            // Transfer additional lamports for rent exemption
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: multisig_authority.to_account_info(),
                        to: large_bitmap_account.to_account_info(),
                    },
                ),
                additional_lamports,
            )?;
        }

        // Expand in chunks of MAX_REALLOC_SIZE (10KB)
        let expansion_size = bytes_needed.min(MAX_REALLOC_SIZE);
        let new_size = current_size + expansion_size;
        
        #[cfg(feature = "debug-logs")]
        msg!("Expanding account by {} bytes to {} bytes", expansion_size, new_size);
        large_bitmap_account.resize(new_size)?;
        
        // Zero out the newly allocated space
        let mut account_data = large_bitmap_account.try_borrow_mut_data()?;
        // Use fill() which may be vectorized by the compiler
        account_data[current_size..new_size].fill(0);
        drop(account_data);
        
        #[cfg(feature = "debug-logs")]
        {
            msg!("Expansion successful, new size: {}", large_bitmap_account.data_len());
            
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
        msg!("Final account size: {}", large_bitmap_account.data_len());
    }

    Ok(())
}