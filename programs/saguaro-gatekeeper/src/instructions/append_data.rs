use anchor_lang::prelude::*;
use crate::AppendData;

/// Handler for appending data to a large bitmap account.
/// 
/// # Compute Optimization
/// - Minimizes logging overhead
/// - Early return for empty data
/// - Direct memory operations
pub fn handler(ctx: Context<AppendData>, data: Vec<u8>) -> Result<()> {
    // Validate data size early to avoid unnecessary loads
    if data.is_empty() {
        return Ok(());
    }
    
    if data.len() > crate::FULL_BITMAP_SIZE_BYTES {
        return err!(crate::GatekeeperError::SlotOutOfRange);
    }
    
    let large_bitmap_account = &ctx.accounts.large_bitmap;
    
    #[cfg(feature = "debug-logs")]
    msg!("Appending {} bytes of data to large bitmap", data.len());
    
    // Get account data for writing without loading first
    let account_info = large_bitmap_account.to_account_info();
    let mut account_data = account_info.try_borrow_mut_data()?;
    
    // Skip the discriminator + epoch + bump + padding (16 bytes total)
    let bitmap_data = &mut account_data[16..];
    
    // Write data to bitmap data section
    let max_write = data.len().min(bitmap_data.len());
    bitmap_data[..max_write].copy_from_slice(&data[..max_write]);
    
    #[cfg(feature = "debug-logs")]
    {
        msg!("Successfully appended {} bytes to large bitmap", max_write);
        // Get epoch from account data directly (bytes 8-9 after discriminator)
        let epoch = u16::from_le_bytes([account_data[8], account_data[9]]);
        msg!("Bitmap epoch: {}", epoch);
    }
    
    Ok(())
}