import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";

/**
 * The PDA seed prefix for the SandwichValidators account.
 * This must match the value in the Rust program.
 */
export const SANDWICH_VALIDATORS_SEED_PREFIX = "sandwich_validators";

/**
 * Maximum number of slots allowed per transaction.
 * This must match the value in the Rust program.
 */
export const MAX_SLOTS_PER_TRANSACTION = 100;

/**
 * Maximum total number of slots allowed per epoch.
 * This must match the value in the Rust program.
 */
export const MAX_SLOTS_PER_EPOCH = 10000;

/**
 * Derives the Program Derived Address (PDA) for the SandwichValidators account.
 *
 * @param multisigAuthority The public key of the multisig authority.
 * @param epoch The epoch number (as a BN).
 * @param programId The program ID.
 * @returns An object containing the PDA public key (`pda`) and the bump seed (`bump`).
 */
export const getSandwichValidatorsPda = (
  multisigAuthority: PublicKey,
  epoch: anchor.BN,
  programId: PublicKey
): { pda: PublicKey; bump: number } => {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [
      Buffer.from(SANDWICH_VALIDATORS_SEED_PREFIX),
      multisigAuthority.toBuffer(),
      epoch.toBuffer("le", 2), // Epoch is u16, so 2 bytes little-endian
    ],
    programId
  );
  return { pda, bump };
};

// --- Instruction Wrapper Functions ---

/**
 * Creates a MethodsBuilder to call the `setSandwichValidators` instruction.
 */
export const setSandwichValidators = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    slots: anchor.BN[];
    multisigAuthority: PublicKey;
  }
) => {
  // Client-side validation to prevent encoding errors
  if (args.slots.length > MAX_SLOTS_PER_TRANSACTION) {
    throw new Error(`TooManySlots: Cannot set more than ${MAX_SLOTS_PER_TRANSACTION} slots per transaction. Got ${args.slots.length}.`);
  }

  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .setSandwichValidators(args.epoch, args.slots)
    .accounts({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    } as any);
};

/**
 * Creates a MethodsBuilder to call the `updateSandwichValidator` instruction.
 * This instruction supports both adding new slots and removing existing slots.
 */
export const updateSandwichValidator = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    newSlots?: anchor.BN[];
    removeSlots?: anchor.BN[];
    multisigAuthority: PublicKey;
  }
) => {
  const newSlots = args.newSlots || [];
  const removeSlots = args.removeSlots || [];

  // Client-side validation to prevent encoding errors
  if (newSlots.length > MAX_SLOTS_PER_TRANSACTION) {
    throw new Error(`TooManySlots: Cannot add more than ${MAX_SLOTS_PER_TRANSACTION} slots per transaction. Got ${newSlots.length}.`);
  }

  if (removeSlots.length > MAX_SLOTS_PER_TRANSACTION) {
    throw new Error(`TooManySlots: Cannot remove more than ${MAX_SLOTS_PER_TRANSACTION} slots per transaction. Got ${removeSlots.length}.`);
  }

  if (newSlots.length === 0 && removeSlots.length === 0) {
    throw new Error(`No operation specified: Must provide either newSlots or removeSlots (or both).`);
  }

  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .updateSandwichValidator(args.epoch, newSlots, removeSlots)
    .accounts({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    } as any);
};


/**
 * Creates a MethodsBuilder to call the `validateSandwichValidators` instruction.
 * 
 * This instruction now automatically derives the current epoch and slot from the Clock sysvar.
 * The instruction will dynamically look up the appropriate SandwichValidators PDA based on
 * the current epoch and validate against the current slot.
 * 
 * Note: The sandwich_validators account must be passed via remainingAccounts.
 * The instruction handler will validate that it matches the expected PDA for the current epoch.
 */
export const validateSandwichValidators = async (
  program: Program<SaguaroGatekeeper>,
  args: {
    multisigAuthority: PublicKey;
  }
) => {
  // Get epoch info from the connection
  const epochInfo = await program.provider.connection.getEpochInfo("processed");
  
  // In local test validators, the epoch is typically 0 regardless of slot
  // The Clock sysvar will also report epoch 0
  const currentEpoch = epochInfo.epoch;
  
  
  // Derive the PDA for the current epoch
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(currentEpoch),
    program.programId
  );
  
  
  return program.methods
    .validateSandwichValidators()
    .accounts({
      multisigAuthority: args.multisigAuthority,
      clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
    } as any)
    .remainingAccounts([
      {
        pubkey: pda,
        isWritable: false,
        isSigner: false,
      },
    ]);
};

/**
 * Creates a MethodsBuilder to call the `closeSandwichValidator` instruction.
 */
export const closeSandwichValidator = (
  program: Program<SaguaroGatekeeper>,
  args: {
    epoch: number;
    multisigAuthority: PublicKey;
  }
) => {
  const { pda } = getSandwichValidatorsPda(
    args.multisigAuthority,
    new anchor.BN(args.epoch),
    program.programId
  );

  return program.methods
    .closeSandwichValidator(args.epoch)
    .accounts({
      sandwichValidators: pda,
      multisigAuthority: args.multisigAuthority,
      systemProgram: SystemProgram.programId,
    } as any);
};

