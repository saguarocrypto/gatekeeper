use anchor_lang::prelude::*;
use crate::AppendData;

pub fn handler(ctx: Context<AppendData>, data: Vec<u8>) -> Result<()> {
    let large_bitmap_account = &ctx.accounts.large_bitmap;
    let large_bitmap = large_bitmap_account.load()?;
    
    msg!("Appending {} bytes of data to large bitmap", data.len());
    
    // Validate data size
    if data.is_empty() {
        msg!("Warning: No data provided to append");
        return Ok(());
    }
    
    if data.len() > crate::FULL_BITMAP_SIZE_BYTES {
        msg!("Error: Data size exceeds maximum bitmap size");
        return err!(crate::GatekeeperError::SlotOutOfRange);
    }
    
    // Get account data for writing
    let account_info = large_bitmap_account.to_account_info();
    let mut account_data = account_info.try_borrow_mut_data()?;
    let bitmap_data = large_bitmap.bitmap_data_mut(&mut account_data);
    
    // Write data to bitmap data section
    let max_write = data.len().min(bitmap_data.len());
    bitmap_data[..max_write].copy_from_slice(&data[..max_write]);
    
    msg!("Successfully appended {} bytes to large bitmap", max_write);
    msg!("Bitmap epoch: {}", large_bitmap.epoch);
    
    Ok(())
}