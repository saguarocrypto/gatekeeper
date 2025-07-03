use anchor_lang::prelude::*;
use crate::AppendDataSandwichValidatorsBitmap;

/// Handler for appending data to a sandwich validators bitmap account.
/// 
/// **Utility Operation**: Raw bitmap data writing
/// This is a low-level utility for writing pre-computed bitmap data.
/// Most users should use `modify_sandwich_validators` instead.
/// 
/// # Compute Optimization
/// - Minimizes logging overhead
/// - Early return for empty data
/// - Direct memory operations
pub fn handler(ctx: Context<AppendDataSandwichValidatorsBitmap>, data: Vec<u8>) -> Result<()> {
    // Validate data size early to avoid unnecessary loads
    if data.is_empty() {
        return Ok(());
    }

    if data.len() > crate::FULL_BITMAP_SIZE_BYTES {
        return err!(crate::GatekeeperError::SlotOutOfRange);
    }

    // First get the current bitmap length
    let current_len = {
        let sandwich_validators = ctx.accounts.sandwich_validators.load()?;
        sandwich_validators.bitmap_len as usize
    };
    
    // Access account raw data for bitmap
    let account_info = ctx.accounts.sandwich_validators.to_account_info();
    let mut account_data = account_info.try_borrow_mut_data()?;

    // Bitmap starts at DATA_OFFSET (discriminator 8 + struct fields 8)
    let bitmap_data = &mut account_data[crate::SandwichValidators::DATA_OFFSET..];
    let available_space = bitmap_data.len().saturating_sub(current_len);

    if data.len() > available_space {
        #[cfg(feature = "debug-logs")]
        msg!(
            "Insufficient space: need {} bytes but only {} available at offset {}",
            data.len(),
            available_space,
            current_len
        );
        return err!(crate::GatekeeperError::SlotOutOfRange);
    }

    // Append the new data at current offset
    bitmap_data[current_len..current_len + data.len()].copy_from_slice(&data);
    
    // Drop the mutable borrow before loading again
    drop(account_data);

    // Update length field
    let sandwich_validators = &mut ctx.accounts.sandwich_validators.load_mut()?;
    sandwich_validators.bitmap_len += data.len() as u32;

    #[cfg(feature = "debug-logs")]
    {
        msg!(
            "Appended {} bytes to bitmap at offset {}",
            data.len(),
            current_len
        );
        msg!("New bitmap_len: {}", sandwich_validators.bitmap_len);
        msg!("Epoch: {}", sandwich_validators.epoch);
    }

    Ok(())
}
