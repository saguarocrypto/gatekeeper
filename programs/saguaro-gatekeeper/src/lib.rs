use anchor_lang::prelude::*;

pub mod constants;
pub mod instructions;

pub const MAX_SLOTS_PER_TRANSACTION: usize = 100;
pub const MAX_SLOTS_PER_EPOCH: usize = 10000;
// Calculate space: 8 (disc) + 2 (epoch) + 1 (bump) + 4 (vec len)
// The actual size includes slot data (slots.len() * 8).
pub const SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE: usize = 8 + 2 + 1 + 4;

// Pause state account size: 8 (discriminator) + 32 (authority) + 1 (is_paused) + 1 (bump)
// This account stores the global pause state for emergency situations.
pub const PAUSE_STATE_ACCOUNT_SIZE: usize = 8 + 32 + 1 + 1;

declare_id!("saGUaroo4mjAcckhEPhtSRthGgFLdQpBvQvuwdf7YG3");

#[program]
pub mod saguaro_gatekeeper {
    use super::*;

    /// Set slots assigned to sandwich facilitating validators for a specific epoch.
    /// This instruction creates or overwrites a PDA containing slot assignments.
    /// 
    /// # Security Notes:
    /// - Requires multisig authority as signer
    /// - Enforces maximum slot limits to prevent DoS attacks
    /// - Manages rent-exemption through proper lamport transfers
    /// - Subject to pause mechanism for emergency situations
    pub fn set_sandwich_validators(
        ctx: Context<SetSandwichValidators>,
        epoch_arg: u16,
        slots_arg: Vec<u64>,
    ) -> Result<()> {
        instructions::set_sandwich_validators_handler(ctx, epoch_arg, slots_arg)
    }

    /// Update slots in an existing SandwichValidators PDA.
    /// Supports both adding new slots and removing existing slots.
    /// 
    /// # Security Notes:
    /// - Requires multisig authority as signer
    /// - Validates PDA existence and authority match
    /// - Subject to pause mechanism for emergency situations
    /// - Validates slot limits and prevents duplicates
    pub fn update_sandwich_validator(
        ctx: Context<UpdateSandwichValidator>,
        epoch_arg: u16,
        new_slots: Vec<u64>,
        remove_slots: Vec<u64>,
    ) -> Result<()> {
        instructions::update_sandwich_validator_handler(ctx, epoch_arg, new_slots, remove_slots)
    }


    /// Validate whether the current slot is gated for sandwich facilitating validators.
    /// This is a public instruction safe for Cross-Program Invocation (CPI).
    /// 
    /// # CPI Safety:
    /// - Returns Ok if PDA doesn't exist (allows normal operation)
    /// - Returns SlotIsGated error only if slot is explicitly gated
    /// - NOT subject to pause mechanism to maintain CPI compatibility
    /// - Always performs validation regardless of system pause state
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
    /// - Subject to pause mechanism for emergency situations
    pub fn close_sandwich_validator(
        ctx: Context<CloseSandwichValidator>,
        epoch_to_close: u16,
    ) -> Result<()> {
        instructions::close_sandwich_validator_handler(ctx, epoch_to_close)
    }

    /// Initialize the pause state for the program.
    /// This creates a global pause state PDA that can be used to pause administrative operations.
    /// 
    /// # Security Notes:
    /// - Can only be called once to initialize the pause state
    /// - Requires multisig authority as signer
    /// - Does not affect the validate instruction to maintain CPI compatibility
    pub fn initialize_pause_state(
        ctx: Context<InitializePauseState>,
    ) -> Result<()> {
        instructions::initialize_pause_state_handler(ctx)
    }

    /// Pause or unpause administrative operations of the program.
    /// When paused, set, update, and close operations will fail gracefully.
    /// 
    /// # Security Notes:
    /// - Only multisig authority can pause/unpause
    /// - Validate instruction remains unaffected for CPI safety
    /// - Fails gracefully with ProgramPaused error for CPI callers
    pub fn set_pause_state(
        ctx: Context<SetPauseState>,
        is_paused: bool,
    ) -> Result<()> {
        instructions::set_pause_state_handler(ctx, is_paused)
    }

    /// Close the pause state PDA and refund rent to the multisig authority.
    /// 
    /// # Security Notes:
    /// - Requires multisig authority as signer and must match stored authority
    /// - Permanently removes pause functionality until re-initialized
    /// - Returns all lamports to the authority account
    pub fn close_pause_state(
        ctx: Context<ClosePauseState>,
    ) -> Result<()> {
        instructions::close_pause_state_handler(ctx)
    }
}

/// Account storing the validator slot assignments for a specific epoch.
/// PDA derived from `SEED_PREFIX`, `multisig_authority` key, and `epoch`.
/// The multisig_authority is not stored as it can be derived from the PDA seeds.
#[account]
#[derive(Debug)]
pub struct SandwichValidators {
    /// The epoch number (u16) to which these slot assignments apply.
    pub epoch: u16,                 // Epoch this PDA is for
    /// A vector of u64 slot numbers that are considered "gated" or assigned for this epoch.
    pub slots: Vec<u64>,            // Slots for this epoch
    /// The canonical bump seed used for PDA derivation.
    pub bump: u8,
}

impl SandwichValidators {
    pub const SEED_PREFIX: &'static [u8] = constants::SEED_PREFIX;
}

/// Accounts for the `set_sandwich_validators` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16, slots_arg: Vec<u64>)]
pub struct SetSandwichValidators<'info> {
    #[account(
        init_if_needed,
        payer = multisig_authority,
        space = SANDWICH_VALIDATORS_ACCOUNT_BASE_SIZE + (slots_arg.len() * 8),
        seeds = [b"sandwich_validators", multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: Account<'info, SandwichValidators>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    /// CHECK: This account is validated in the pause_utils::check_not_paused function
    #[account(
        seeds = [b"pause_state"],
        bump
    )]
    pub pause_state: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `validate_sandwich_validators` instruction.
/// Derives current epoch/slot from Clock sysvar and validates PDA manually.
#[derive(Accounts)]
pub struct ValidateSandwichValidators<'info> {
    /// The multisig authority account used in PDA derivation.
    /// Not a signer as validation is public.
    /// CHECK: This is used for PDA derivation only
    pub multisig_authority: AccountInfo<'info>,
    /// The Clock sysvar to get current epoch and slot
    pub clock: Sysvar<'info, Clock>,
}

/// Accounts for the `update_sandwich_validator` instruction.
#[derive(Accounts)]
#[instruction(epoch_arg: u16, new_slots: Vec<u64>, remove_slots: Vec<u64>)]
pub struct UpdateSandwichValidator<'info> {
    #[account(
        mut,
        seeds = [b"sandwich_validators", multisig_authority.key().as_ref(), &epoch_arg.to_le_bytes()],
        bump,
        constraint = sandwich_validators.epoch == epoch_arg @ GatekeeperError::EpochMismatch
    )]
    pub sandwich_validators: Account<'info, SandwichValidators>,

    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    /// CHECK: This account is validated in the pause_utils::check_not_paused function
    #[account(
        seeds = [b"pause_state"],
        bump
    )]
    pub pause_state: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}


/// Accounts for the `close_sandwich_validator` instruction.
#[derive(Accounts)]
#[instruction(epoch_to_close: u16)]
pub struct CloseSandwichValidator<'info> {
    #[account(
        mut,
        close = multisig_authority,
        seeds = [b"sandwich_validators", multisig_authority.key().as_ref(), &epoch_to_close.to_le_bytes()],
        bump
    )]
    pub sandwich_validators: Account<'info, SandwichValidators>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    /// CHECK: This account is validated in the pause_utils::check_not_paused function
    #[account(
        seeds = [b"pause_state"],
        bump
    )]
    pub pause_state: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `initialize_pause_state` instruction.
#[derive(Accounts)]
pub struct InitializePauseState<'info> {
    #[account(
        init,
        payer = multisig_authority,
        space = PAUSE_STATE_ACCOUNT_SIZE,
        seeds = [b"pause_state"],
        bump
    )]
    pub pause_state: Account<'info, PauseState>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Accounts for the `set_pause_state` instruction.
#[derive(Accounts)]
#[instruction(is_paused: bool)]
pub struct SetPauseState<'info> {
    #[account(
        mut,
        seeds = [b"pause_state"],
        bump = pause_state.bump,
        constraint = pause_state.multisig_authority == multisig_authority.key() @ GatekeeperError::AuthorityMismatch
    )]
    pub pause_state: Account<'info, PauseState>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
}

/// Accounts for the `close_pause_state` instruction.
#[derive(Accounts)]
pub struct ClosePauseState<'info> {
    #[account(
        mut,
        seeds = [b"pause_state"],
        bump = pause_state.bump,
        close = multisig_authority,
        constraint = pause_state.multisig_authority == multisig_authority.key() @ GatekeeperError::AuthorityMismatch
    )]
    pub pause_state: Account<'info, PauseState>,
    #[account(mut)]
    pub multisig_authority: Signer<'info>,
}

/// Account storing the global pause state for the program.
#[account]
#[derive(Debug)]
pub struct PauseState {
    /// The public key of the multisig authority responsible for managing this entry.
    pub multisig_authority: Pubkey, // Squads multisig account
    /// The canonical bump seed used for PDA derivation.
    pub bump: u8,
    /// A boolean indicating whether the program is paused.
    pub is_paused: bool,
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

#[event]
pub struct PauseStateChanged {
    pub authority: Pubkey,
    pub is_paused: bool,
}

#[event]
pub struct PauseStateInitialized {
    pub authority: Pubkey,
}

#[event]
pub struct PauseStateClosed {
    pub authority: Pubkey,
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
    #[msg("The program is currently paused and cannot perform this operation.")]
    ProgramPaused,
    #[msg("Slot number is outside the acceptable range.")]
    SlotOutOfRange,
    #[msg("Missing sandwich validators account in remaining_accounts.")]
    MissingSandwichValidatorsAccount,
    #[msg("Invalid sandwich validators PDA provided.")]
    InvalidSandwichValidatorsPDA,
}