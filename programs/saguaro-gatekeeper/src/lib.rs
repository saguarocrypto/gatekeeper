#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use anchor_lang::prelude::*;

pub mod constants;
pub mod instructions;

// Re-export all constants for backward compatibility
pub use constants::*;

declare_id!("saGUaroo4mjAcckhEPhtSRthGgFLdQpBvQvuwdf7YG3");

#[program]
pub mod saguaro_gatekeeper {
    use super::*;

    /// Create a SandwichValidators account for a specific epoch.
    /// This instruction only creates the account with initial 10KB size - no slots are set.
    /// 
    /// **CRUD Operation: CREATE**
    /// 
    /// # Security Notes:
    /// - Requires multisig authority as signer
    /// - Manages rent-exemption through proper lamport transfers
    /// - Account starts with all slots ungated (bitmap zeroed)
    pub fn set_sandwich_validators(
        ctx: Context<SetSandwichValidators>,
        epoch_arg: u16,
    ) -> Result<()> {
        instructions::set_sandwich_validators_handler(ctx, epoch_arg)
    }

    /// Modify slots in an existing SandwichValidators PDA.
    /// Supports both gating slots (set to true) and ungating slots (set to false).
    /// 
    /// **CRUD Operation: UPDATE**
    /// 
    /// # Security Notes:
    /// - Requires multisig authority as signer
    /// - Validates PDA existence and authority match
    /// - Validates slot limits and prevents duplicates
    pub fn modify_sandwich_validators(
        ctx: Context<ModifySandwichValidators>,
        epoch_arg: u16,
        slots_to_gate: Vec<u64>,
        slots_to_ungate: Vec<u64>,
    ) -> Result<()> {
        instructions::modify_sandwich_validators_handler(ctx, epoch_arg, slots_to_gate, slots_to_ungate)
    }


    /// Validate whether the current slot is gated for sandwich facilitating validators.
    /// This is a public instruction safe for Cross-Program Invocation (CPI).
    /// 
    /// # CPI Safety:
    /// - Returns Ok if PDA doesn't exist (allows normal operation)
    /// - Returns SlotIsGated error only if slot is explicitly gated
    /// - Always performs validation for CPI compatibility
    /// - Derives current epoch and slot from Clock sysvar internally
    pub fn validate_sandwich_validators(
        ctx: Context<ValidateSandwichValidators>,
    ) -> Result<()> {
        instructions::validate_sandwich_validators_handler(ctx)
    }

    /// Close a SandwichValidators PDA for a past epoch and refund rent.
    /// 
    /// # Security Notes:
    /// - Only allows closing PDAs for past epochs
    /// - Requires multisig authority as signer
    pub fn close_sandwich_validator(
        ctx: Context<CloseSandwichValidator>,
        epoch_to_close: u16,
    ) -> Result<()> {
        instructions::close_sandwich_validator_handler(ctx, epoch_to_close)
    }




    /// Expand the sandwich validators bitmap account to full size without writing data.
    /// This is used to expand the account beyond the initial 10KB limit.
    pub fn expand_sandwich_validators_bitmap(
        ctx: Context<ExpandSandwichValidatorsBitmap>,
        epoch_arg: u16,
    ) -> Result<()> {
        instructions::expand_sandwich_validators_bitmap_handler(ctx, epoch_arg)
    }

    /// Append data to the sandwich validators bitmap account.
    /// This is a low-level utility for writing pre-computed bitmap data.
    /// Most users should use `modify_sandwich_validators` instead.
    pub fn append_data_sandwich_validators_bitmap(
        ctx: Context<AppendDataSandwichValidatorsBitmap>,
        epoch_arg: u16,
        data: Vec<u8>,
    ) -> Result<()> {
        instructions::append_data_sandwich_validators_bitmap_handler(ctx, epoch_arg, data)
    }

    /// Clear all data in the sandwich validators bitmap account.
    /// This sets all slots in the bitmap to ungated (false).
    pub fn clear_data_sandwich_validators_bitmap(
        ctx: Context<ClearDataSandwichValidatorsBitmap>,
        epoch_arg: u16,
    ) -> Result<()> {
        instructions::clear_data_sandwich_validators_bitmap_handler(ctx, epoch_arg)
    }

}


/// Account storing the validator slot assignments for a specific epoch using a bitmap.
/// Uses zero-copy patterns for efficient access to 432,000 slots. Data is accessed manually to avoid heap allocation issues.
#[account(zero_copy)]
#[repr(C)]
pub struct SandwichValidators {
    /// The epoch number (u16) to which these slot assignments apply.
    pub epoch: u16,          // 2 bytes
    pub bump: u8,            // 1 byte
    pub _padding: [u8; 1],   // 1 byte to align `bitmap_len` to 4-byte boundary
    pub bitmap_len: u32,     // 4 bytes
    // Total struct size: 8 bytes (2 + 1 + 1 + 4)
    // With 8-byte discriminator, bitmap data begins at offset 16
}

impl SandwichValidators {
    pub const SEED_PREFIX: &'static [u8] = b"sandwich_validators";
    pub const DATA_OFFSET: usize = 16; // discriminator (8) + epoch (2) + bump (1) + padding (1) + bitmap_len (4)
}

/// Accounts for the `set_sandwich_validators` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16)]
pub struct SetSandwichValidators<'info> {
    /// CHECK: This account is manually validated and initialized in the instruction handler
    #[account(
        mut,
        seeds = [SandwichValidators::SEED_PREFIX, multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: AccountInfo<'info>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `validate_sandwich_validators` instruction.
#[derive(Accounts)]
pub struct ValidateSandwichValidators<'info> {
    /// The PDA account to be validated.
    /// CHECK: The address is manually validated in the instruction handler against the
    /// multisig_authority and current epoch from the clock sysvar.
    pub sandwich_validators: AccountInfo<'info>,
    /// The multisig authority account used in PDA derivation.
    /// Not a signer as validation is public.
    /// CHECK: This is used for PDA derivation only and is not a signer.
    pub multisig_authority: AccountInfo<'info>,
    /// The Clock sysvar to get current epoch and slot.
    pub clock: Sysvar<'info, Clock>,
}

/// Accounts for the `modify_sandwich_validators` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16, slots_to_gate: Vec<u64>, slots_to_ungate: Vec<u64>)]
pub struct ModifySandwichValidators<'info> {
    /// CHECK: This account is manually validated in the instruction handler
    #[account(
        mut,
        seeds = [SandwichValidators::SEED_PREFIX, multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: AccountInfo<'info>,

    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}



/// Accounts for the `close_sandwich_validator` instruction.
#[derive(Accounts)]
#[instruction(epoch_to_close: u16)]
pub struct CloseSandwichValidator<'info> {
    #[account(
        mut,
        close = multisig_authority,
        seeds = [SandwichValidators::SEED_PREFIX, multisig_authority.key().as_ref(), &epoch_to_close.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: AccountLoader<'info, SandwichValidators>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}


/// Accounts for the `expand_sandwich_validators_bitmap` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16)]
pub struct ExpandSandwichValidatorsBitmap<'info> {
    /// CHECK: This account is validated through PDA derivation with seeds constraint
    /// and additional epoch validation in the instruction handler
    #[account(
        mut,
        seeds = [SandwichValidators::SEED_PREFIX, multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: AccountInfo<'info>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `append_data_sandwich_validators_bitmap` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16)]
pub struct AppendDataSandwichValidatorsBitmap<'info> {
    #[account(
        mut,
        seeds = [SandwichValidators::SEED_PREFIX, multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: AccountLoader<'info, SandwichValidators>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
}

/// Accounts for the `clear_data_sandwich_validators_bitmap` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16)]
pub struct ClearDataSandwichValidatorsBitmap<'info> {
    #[account(
        mut,
        seeds = [SandwichValidators::SEED_PREFIX, multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: AccountLoader<'info, SandwichValidators>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
}


/// Events emitted by the Saguaro Gatekeeper program for monitoring
#[event]
pub struct SandwichValidatorsSet {
    pub authority: Pubkey,
    pub epoch: u16,
    pub slot_count: u16,
}

#[event]
pub struct SandwichValidatorsUpdated {
    pub authority: Pubkey,
    pub epoch: u16,
    pub slots_added: u16,
    pub slots_removed: u16,
    pub total_slots: u16,
}

#[event]
pub struct SandwichValidatorsClosed {
    pub authority: Pubkey,
    pub epoch: u16,
}


/// Custom error codes for the Saguaro Gatekeeper program.
#[error_code]
pub enum GatekeeperError {
    #[msg("The current slot is gated and cannot be used.")]
    SlotIsGated,
    #[msg("Epoch in PDA does not match current validation epoch.")]
    EpochMismatch,
    #[msg("Authority in PDA does not match provided authority.")]
    AuthorityMismatch,
    #[msg("Invalid authority provided for the operation.")]
    InvalidAuthority,
    #[msg("Invalid PDA provided for the operation.")]
    InvalidPda,
    #[msg("An invalid epoch was provided for the operation.")]
    InvalidEpoch,
    #[msg("The epoch has not finished yet and cannot be closed.")]
    EpochNotFinished,
    #[msg("The provided list of slots exceeds the maximum allowed size.")]
    TooManySlots,
    #[msg("Account bump does not match expected value.")]
    BumpMismatch,
    #[msg("Account rent exempt balance not met after transfer.")]
    RentNotMet,
    #[msg("Slot numbers must be unique within an epoch.")]
    DuplicateSlots,
    #[msg("Empty slot list may not be intentional.")]
    EmptySlotList,
    #[msg("Slot number is outside the acceptable range.")]
    SlotOutOfRange,
    #[msg("Slot numbers overlap between slots_to_gate and slots_to_ungate.")]
    OverlapSlots,
    #[msg("Missing sandwich validators account in remaining_accounts.")]
    MissingSandwichValidatorsAccount,
    #[msg("Invalid sandwich validators PDA provided.")]
    InvalidSandwichValidatorsPDA,
}