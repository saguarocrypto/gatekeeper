use anchor_lang::prelude::*;
use crate::ClearData;

/// Handler for clearing all data in a large bitmap account.
/// 
/// # Compute Optimization
/// - Minimizes logging overhead
/// - Uses efficient memory clearing operations
pub fn handler(ctx: Context<ClearData>) -> Result<()> {
    let large_bitmap_account = &ctx.accounts.large_bitmap;
    
    // Get account data for writing without loading first
    let account_info = large_bitmap_account.to_account_info();
    let mut account_data = account_info.try_borrow_mut_data()?;
    
    #[cfg(feature = "debug-logs")]
    {
        // Get epoch from account data directly (bytes 8-9 after discriminator)
        let epoch = u16::from_le_bytes([account_data[8], account_data[9]]);
        msg!("Clearing all data in large bitmap for epoch {}", epoch);
    }
    
    // Skip the discriminator + epoch + bump + padding (16 bytes total) and clear bitmap data
    let bitmap_data = &mut account_data[16..];
    bitmap_data.fill(0);
    
    #[cfg(feature = "debug-logs")]
    {
        msg!("Large bitmap data cleared successfully");
        msg!("All {} bytes of slot data reset to 0", crate::FULL_BITMAP_SIZE_BYTES);
    }
    
    Ok(())
}