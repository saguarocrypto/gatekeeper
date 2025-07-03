use anchor_lang::prelude::*;
use crate::ClearDataSandwichValidatorsBitmap;

/// Handler for clearing all data in a sandwich validators bitmap account.
/// 
/// **Utility Operation**: Clear all bitmap data (ungate all slots)
/// This sets all slots in the bitmap to ungated (false) and resets bitmap_len to 0.
/// 
/// # Compute Optimization
/// - Minimizes logging overhead
/// - Uses efficient memory clearing operations
pub fn handler(ctx: Context<ClearDataSandwichValidatorsBitmap>, _epoch_arg: u16) -> Result<()> {
    // Get account data for writing
    let account_info = ctx.accounts.sandwich_validators.to_account_info();
    let mut account_data = account_info.try_borrow_mut_data()?;
    
    #[cfg(feature = "debug-logs")]
    {
        // Get epoch from account data directly (bytes 8-9 after discriminator)
        let epoch = u16::from_le_bytes([account_data[8], account_data[9]]);
        msg!("Clearing all data in large bitmap for epoch {}", epoch);
    }
    
    // Clear bitmap data starting at DATA_OFFSET (16 bytes)
    let bitmap_data = &mut account_data[crate::SandwichValidators::DATA_OFFSET..];
    bitmap_data.fill(0);
    
    // Drop the raw data borrow before using load_mut
    drop(account_data);
    
    // Reset the bitmap_len field to 0
    let sandwich_validators = &mut ctx.accounts.sandwich_validators.load_mut()?;
    sandwich_validators.bitmap_len = 0;
    
    #[cfg(feature = "debug-logs")]
    {
        msg!("Large bitmap data cleared successfully");
        msg!("All bitmap data reset to 0 and bitmap_len reset to 0");
    }
    
    Ok(())
}