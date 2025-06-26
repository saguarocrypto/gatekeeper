use anchor_lang::prelude::*;
use crate::ClearData;

pub fn handler(ctx: Context<ClearData>) -> Result<()> {
    let large_bitmap_account = &ctx.accounts.large_bitmap;
    let large_bitmap = large_bitmap_account.load()?;
    
    msg!("Clearing all data in large bitmap for epoch {}", large_bitmap.epoch);
    
    // Get account data for writing
    let account_info = large_bitmap_account.to_account_info();
    let mut account_data = account_info.try_borrow_mut_data()?;
    
    // Clear all slots in the bitmap
    large_bitmap.clear_all_slots(&mut account_data);
    
    msg!("Large bitmap data cleared successfully");
    msg!("All {} bytes of slot data reset to 0", crate::FULL_BITMAP_SIZE_BYTES);
    
    Ok(())
}