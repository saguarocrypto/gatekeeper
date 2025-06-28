import * as anchor from "@coral-xyz/anchor";
import { web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  setSandwichValidators,
  // expandSandwichValidators, // REMOVED: Function was removed from contract and SDK
  updateSandwichValidator,
  validateSandwichValidators,
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

async function main() {
  console.log("=== Debug Validation Test ===\n");

  try {
    // Setup
    const connection = new web3.Connection("https://api.devnet.solana.com", "confirmed");
    const walletPath = process.env.WALLET_PATH || `${process.env.HOME}/.config/solana/id.json`;
    const keypair = web3.Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(readFileSync(walletPath, "utf-8")))
    );
    const wallet = new anchor.Wallet(keypair);
    
    const provider = new anchor.AnchorProvider(connection, wallet, {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
    });
    anchor.setProvider(provider);

    const program = anchor.workspace.SaguaroGatekeeper as anchor.Program<SaguaroGatekeeper>;
    console.log(`Program ID: ${program.programId.toBase58()}`);

    // Get current slot info
    const currentSlot = await connection.getSlot();
    const currentEpoch = Math.floor(currentSlot / SLOTS_PER_EPOCH);
    const epochStartSlot = currentEpoch * SLOTS_PER_EPOCH;
    const currentSlotOffset = currentSlot - epochStartSlot;
    
    console.log(`Current slot: ${currentSlot}`);
    console.log(`Current epoch: ${currentEpoch}`);
    console.log(`Slot offset: ${currentSlotOffset}`);
    console.log(`Required bitmap size for current slot: ${Math.ceil(currentSlotOffset / 8)} bytes`);

    // Test with a smaller, controllable offset
    const testEpoch = 500; // Use a test epoch
    const testEpochStart = testEpoch * SLOTS_PER_EPOCH;
    const testSlotOffset = 5000; // 5000 slots into the epoch
    const testSlot = testEpochStart + testSlotOffset;
    
    console.log(`\nTest slot: ${testSlot}`);
    console.log(`Test epoch: ${testEpoch}`);
    console.log(`Test slot offset: ${testSlotOffset}`);
    console.log(`Required bitmap size for test slot: ${Math.ceil(testSlotOffset / 8)} bytes`);

    // Create PDA with test slot gated
    console.log("\n1. Creating PDA with gated test slot...");
    await setSandwichValidators(program, {
      epoch: testEpoch,
      slots: [new BN(testSlot)],
      multisigAuthority: wallet.publicKey,
    })
      .signers([wallet.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(wallet.publicKey, new BN(testEpoch), program.programId);
    const pdaInfo = await connection.getAccountInfo(pda);
    console.log(`PDA created with size: ${pdaInfo?.data.length} bytes`);

    // Try to validate - this should be a manual check since we can't control current slot
    console.log("\n2. Checking PDA structure...");
    const account = await program.account.sandwichValidators.fetch(pda);
    console.log(`Epoch: ${account.epoch}`);
    console.log(`Bump: ${account.bump}`);
    const bitmapDataLength = pdaInfo.data.length - 16; // Header is 16 bytes for SandwichValidators
    console.log(`Bitmap data length: ${bitmapDataLength} bytes`);
    console.log(`Max trackable slots: ${bitmapDataLength * 8}`);

    // Check if the bit is actually set
    const byteIndex = Math.floor(testSlotOffset / 8);
    const bitIndex = testSlotOffset % 8;
    console.log(`\n3. Checking bit at byte ${byteIndex}, bit ${bitIndex}:`);
    
    // For SandwichValidators, we need to access the raw data after the header (16 bytes)
    const bitmapStartOffset = 16; // 8 (discriminator) + 2 (epoch) + 1 (bump) + 5 (padding)
    if (byteIndex < bitmapDataLength) {
      const byte = pdaInfo.data[bitmapStartOffset + byteIndex];
      const bitSet = (byte >> bitIndex) & 1;
      console.log(`Byte value: 0x${byte.toString(16).padStart(2, '0')} (${byte})`);
      console.log(`Bit ${bitIndex} is: ${bitSet ? 'SET (gated)' : 'UNSET (not gated)'}`);
    } else {
      console.log(`Byte index ${byteIndex} is beyond bitmap length ${bitmapDataLength}`);
    }

  } catch (error) {
    console.error(`Error: ${error}`);
  }
}

main().catch(console.error);