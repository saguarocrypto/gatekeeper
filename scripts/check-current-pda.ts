import * as anchor from "@coral-xyz/anchor";
import { web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

async function main() {
  console.log("=== Check Current Epoch PDA ===\n");

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

    // Check if PDA exists for current epoch
    const { pda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
    const accountInfo = await connection.getAccountInfo(pda);
    
    if (accountInfo) {
      console.log(`\nPDA exists with size: ${accountInfo.data.length} bytes`);
      
      // Read the raw account data to see the bitmap length
      const discriminator = accountInfo.data.readBigUInt64LE(0);
      const epoch = accountInfo.data.readUInt16LE(8);
      const bitmapLen = accountInfo.data.readUInt32LE(10);
      const bump = accountInfo.data[accountInfo.data.length - 1];
      
      console.log(`Raw data - Discriminator: ${discriminator.toString(16)}`);
      console.log(`Raw data - Epoch: ${epoch}`);
      console.log(`Raw data - Bitmap length: ${bitmapLen} bytes`);
      console.log(`Raw data - Max trackable slots: ${bitmapLen * 8}`);
      console.log(`Raw data - Bump: ${bump}`);
      console.log(`Raw data - Header size: 15 bytes`);
      console.log(`Raw data - Expected account size: ${15 + bitmapLen + 1} bytes`);
      console.log(`Raw data - Actual account size: ${accountInfo.data.length} bytes`);
      
      // Try to fetch using program account decoder
      try {
        const account = await program.account.sandwichValidators.fetch(pda);
        console.log(`\nProgram decode - Epoch: ${account.epoch}`);
        console.log(`Program decode - Bitmap length: ${account.slots.length} bytes`);
        console.log(`Program decode - Max trackable slots: ${account.slots.length * 8}`);
      } catch (error) {
        console.log(`\nProgram decode failed: ${error}`);
      }
    } else {
      console.log(`\nNo PDA exists for current epoch ${currentEpoch}`);
    }

  } catch (error) {
    console.error(`Error: ${error}`);
  }
}

main().catch(console.error);