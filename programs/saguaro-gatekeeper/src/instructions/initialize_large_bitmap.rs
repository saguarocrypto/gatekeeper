use anchor_lang::prelude::*;
use anchor_lang::system_program;
use crate::{InitializeLargeBitmap, LargeBitmap, INITIAL_ACCOUNT_SIZE};

pub fn handler(ctx: Context<InitializeLargeBitmap>, epoch_arg: u16) -> Result<()> {
    let multisig_authority = &ctx.accounts.multisig_authority;
    let large_bitmap_account = &ctx.accounts.large_bitmap;
    let system_program = &ctx.accounts.system_program;

    // Calculate rent for initial account size
    let rent = Rent::get()?;
    let required_lamports = rent.minimum_balance(INITIAL_ACCOUNT_SIZE);

    #[cfg(feature = "debug-logs")]
    {
        msg!("Initializing large bitmap account with {} bytes", INITIAL_ACCOUNT_SIZE);
        msg!("Required lamports: {}", required_lamports);
    }

    // Create the account with initial 10KB allocation
    let bump = ctx.bumps.large_bitmap;
    let bump_seed = [bump];
    let epoch_bytes = epoch_arg.to_le_bytes();
    let authority_key = multisig_authority.key();
    let seeds = &[
        LargeBitmap::SEED_PREFIX,
        authority_key.as_ref(),
        &epoch_bytes,
        &bump_seed,
    ];
    let signer_seeds = &[&seeds[..]];

    system_program::create_account(
        CpiContext::new_with_signer(
            system_program.to_account_info(),
            system_program::CreateAccount {
                from: multisig_authority.to_account_info(),
                to: large_bitmap_account.to_account_info(),
            },
            signer_seeds,
        ),
        required_lamports,
        INITIAL_ACCOUNT_SIZE as u64,
        &crate::ID,
    )?;

    // Initialize the account data
    let mut account_data = large_bitmap_account.try_borrow_mut_data()?;
    
    // Write discriminator (first 8 bytes)
    let discriminator = LargeBitmap::DISCRIMINATOR;
    account_data[0..8].copy_from_slice(&discriminator);
    
    // Initialize epoch (bytes 8-9)
    account_data[8..10].copy_from_slice(&epoch_arg.to_le_bytes());
    
    // Initialize bump (byte 10)
    account_data[10] = bump;
    
    // Initialize padding (bytes 11-15)
    account_data[11..16].fill(0);
    
    // Zero out the bitmap data (remaining bytes)
    // Use fill() which may be vectorized by the compiler
    account_data[16..].fill(0);

    #[cfg(feature = "debug-logs")]
    {
        msg!("Large bitmap account initialized successfully");
        msg!("Account size: {} bytes", account_data.len());
        msg!("Epoch: {}, Bump: {}", epoch_arg, bump);
    }

    Ok(())
}