use anchor_lang::prelude::*;
use crate::ClearData;

/// Handler for clearing all data in a large bitmap account.
/// 
/// # Compute Optimization
/// - Minimizes logging overhead
/// - Uses efficient memory clearing operations
pub fn handler(ctx: Context<ClearData>) -> Result<()> {
    let large_bitmap_account = &ctx.accounts.large_bitmap;
    let large_bitmap = large_bitmap_account.load()?;
    
    #[cfg(feature = "debug-logs")]
    msg!("Clearing all data in large bitmap for epoch {}", large_bitmap.epoch);
    
    // Get account data for writing
    let account_info = large_bitmap_account.to_account_info();
    let mut account_data = account_info.try_borrow_mut_data()?;
    
    // Clear all slots in the bitmap
    large_bitmap.clear_all_slots(&mut account_data);
    
    #[cfg(feature = "debug-logs")]
    {
        msg!("Large bitmap data cleared successfully");
        msg!("All {} bytes of slot data reset to 0", crate::FULL_BITMAP_SIZE_BYTES);
    }
    
    Ok(())
}