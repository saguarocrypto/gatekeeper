use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::{ExpandSandwichValidators, FULL_BITMAP_SIZE_BYTES, SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE, MAX_REALLOC_SIZE};

/// Handler for expanding a SandwichValidators PDA to full bitmap capacity.
/// 
/// This instruction expands the existing sandwich_validators PDA from its initial
/// 10KB size to the full bitmap size needed to track all slots in an epoch.
/// 
/// # Compute Optimization
/// - Uses realloc() to expand account size incrementally
/// - Direct memory operations for bitmap initialization
/// - Minimal logging overhead
pub fn handler(ctx: Context<ExpandSandwichValidators>, _epoch_arg: u16) -> Result<()> {
    let sandwich_validators = &ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;

    // Validate account exists and is owned by our program
    if sandwich_validators.data_is_empty() || *sandwich_validators.owner != *ctx.program_id {
        return err!(crate::GatekeeperError::InvalidPda);
    }

    #[cfg(feature = "debug-logs")]
    {
        msg!("Expanding sandwich_validators PDA for epoch {}", epoch_arg);
        msg!("Current account size: {}", sandwich_validators.data_len());
    }

    // Calculate target size: base + full bitmap
    let target_size = SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE + FULL_BITMAP_SIZE_BYTES;
    let current_size = sandwich_validators.data_len();
    let bytes_needed = target_size.saturating_sub(current_size);
    
    if bytes_needed == 0 {
        #[cfg(feature = "debug-logs")]
        msg!("Account is already at full size");
        
        // Even if the account is the right size, we may need to fix the bitmap length field
        let mut account_data = sandwich_validators.try_borrow_mut_data()?;
        const HEADER_SIZE: usize = 8 + 2 + 4; // discriminator + epoch + vec_len
        
        let current_bitmap_len = u32::from_le_bytes([
            account_data[10], account_data[11], account_data[12], account_data[13]
        ]) as usize;
        
        if current_bitmap_len != FULL_BITMAP_SIZE_BYTES {
            #[cfg(feature = "debug-logs")]
            msg!("Fixing bitmap length from {} to {}", current_bitmap_len, FULL_BITMAP_SIZE_BYTES);
            
            // Update bitmap length to full size
            account_data[10..14].copy_from_slice(&(FULL_BITMAP_SIZE_BYTES as u32).to_le_bytes());
        }
        
        drop(account_data);
        return Ok(());
    }

    #[cfg(feature = "debug-logs")]
    msg!("Need to expand by {} bytes to reach target size of {}", bytes_needed, target_size);
    
    // Calculate rent for target size
    let rent = Rent::get()?;
    let target_lamports = rent.minimum_balance(target_size);
    let current_lamports = sandwich_validators.lamports();
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
                    to: sandwich_validators.to_account_info(),
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
    
    sandwich_validators.resize(new_size)?;
    
    // Zero out the newly allocated space (bitmap data)
    let mut account_data = sandwich_validators.try_borrow_mut_data()?;
    
    // Account structure: discriminator(8) + epoch(2) + vec_len(4) + bitmap_data + bump(1)
    const HEADER_SIZE: usize = 8 + 2 + 4; // discriminator + epoch + vec_len
    
    // Calculate the new bitmap length based on the new account size
    let current_bitmap_len = u32::from_le_bytes([
        account_data[10], account_data[11], account_data[12], account_data[13]
    ]) as usize;
    
    // Calculate the correct bitmap length based on the new account size
    // Account structure: discriminator(8) + epoch(2) + vec_len(4) + bitmap_data + bump(1)
    // So bitmap_data = account_size - (8 + 2 + 4 + 1) = account_size - 15
    let new_bitmap_len = new_size.saturating_sub(HEADER_SIZE + 1); // subtract 1 for bump
    
    // Cap at full bitmap size
    let final_bitmap_len = new_bitmap_len.min(FULL_BITMAP_SIZE_BYTES);
    account_data[10..14].copy_from_slice(&(final_bitmap_len as u32).to_le_bytes());
    
    // Zero out the newly allocated bitmap space
    let bitmap_start = HEADER_SIZE + current_bitmap_len;
    let bitmap_end = HEADER_SIZE + final_bitmap_len;
    
    if bitmap_end <= account_data.len() - 1 { // Leave space for bump at the end
        account_data[bitmap_start..bitmap_end].fill(0);
        
        // Update bump position to the end of the expanded account
        let bump_pos = new_size - 1;
        if bump_pos < account_data.len() {
            // Keep the existing bump value
            let bump = account_data[current_size - 1];
            account_data[bump_pos] = bump;
        }
    }
    
    drop(account_data);
    
    #[cfg(feature = "debug-logs")]
    {
        msg!("Expansion successful, new size: {}", sandwich_validators.data_len());
        
        // If we haven't reached target size yet, caller needs to invoke this instruction again
        if new_size < target_size {
            msg!("Account needs further expansion. Current: {}, Target: {}", new_size, target_size);
        } else {
            msg!("Account fully expanded to handle all epoch slots");
        }
    }

    Ok(())
}