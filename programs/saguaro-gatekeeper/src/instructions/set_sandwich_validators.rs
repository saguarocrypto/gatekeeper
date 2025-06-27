use anchor_lang::prelude::*;
use crate::{GatekeeperError, SetSandwichValidators, SandwichValidatorsSet, INITIAL_ACCOUNT_SIZE, SandwichValidators};

/// Handler for the `set_sandwich_validators` instruction.
/// 
/// **CRUD Operation: CREATE**
/// This instruction only creates the SandwichValidators account with initial 10KB size.
/// No slots are set - use `modify_sandwich_validators` for slot operations.
/// Use `expand_sandwich_validators_bitmap` to expand to full epoch size.
/// 
/// # Compute Optimization
/// This handler focuses solely on account creation:
/// - Direct account creation with System Program
/// - Minimal memory allocations
/// - No slot processing overhead
pub fn handler(ctx: Context<SetSandwichValidators>, epoch_arg: u16) -> Result<()> {
    let sandwich_validators_ai = &ctx.accounts.sandwich_validators;
    let multisig_authority = &ctx.accounts.multisig_authority;
    let system_program = &ctx.accounts.system_program;

    // Ensure account doesn't already exist
    if !sandwich_validators_ai.data_is_empty() {
        return err!(GatekeeperError::InvalidPda);
    }

    // Create the account with initial size (10KB limit for System Program)
    let rent = Rent::get()?;
    let account_size = INITIAL_ACCOUNT_SIZE; // Start with 10KB due to Solana limitations
    let lamports = rent.minimum_balance(account_size);
    
    // Create account using direct invoke_signed
    let create_account_ix = anchor_lang::solana_program::system_instruction::create_account(
        multisig_authority.key,
        sandwich_validators_ai.key,
        lamports,
        account_size as u64,
        ctx.program_id,
    );
    
    anchor_lang::solana_program::program::invoke_signed(
        &create_account_ix,
        &[
            multisig_authority.to_account_info(),
            sandwich_validators_ai.to_account_info(),
            system_program.to_account_info(),
        ],
        &[&[
            SandwichValidators::SEED_PREFIX,
            multisig_authority.key().as_ref(),
            &epoch_arg.to_le_bytes(),
            &[ctx.bumps.sandwich_validators],
        ]],
    )?;
    
    #[cfg(feature = "debug-logs")]
    msg!("Created SandwichValidators account with initial size: {} bytes", account_size);

    // Initialize account structure
    let mut data = sandwich_validators_ai.try_borrow_mut_data()?;
    
    // SandwichValidators structure: discriminator (8) + epoch (2) + bump (1) + padding (5) + bitmap data
    const HEADER_SIZE: usize = 16; // SandwichValidators::DATA_OFFSET
    
    // Write discriminator - use Anchor's generated discriminator for SandwichValidators
    use anchor_lang::Discriminator;
    data[0..8].copy_from_slice(&SandwichValidators::DISCRIMINATOR);
    
    // Write epoch
    data[8..10].copy_from_slice(&epoch_arg.to_le_bytes());
    
    // Write bump
    data[10] = ctx.bumps.sandwich_validators;
    
    // Clear padding bytes
    data[11..16].fill(0);
    
    // Initialize bitmap area to zero (all slots ungated by default)
    // Use fill() which may be vectorized by the compiler
    data[HEADER_SIZE..].fill(0);
    
    drop(data);
    
    // Emit event for monitoring
    emit!(SandwichValidatorsSet {
        authority: *multisig_authority.key,
        epoch: epoch_arg,
        slot_count: 0, // No slots set during creation
    });
    
    Ok(())
}