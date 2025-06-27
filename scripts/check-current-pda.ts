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
      
      // Read the raw account data using the new SandwichValidators structure
      const discriminator = accountInfo.data.readBigUInt64LE(0);
      const epoch = accountInfo.data.readUInt16LE(8);
      const bump = accountInfo.data.readUInt8(10);
      const bitmapDataLength = accountInfo.data.length - 16; // 8 (discriminator) + 2 (epoch) + 1 (bump) + 5 (padding) = 16 bytes header
      
      console.log(`Raw data - Discriminator: ${discriminator.toString(16)}`);
      console.log(`Raw data - Epoch: ${epoch}`);
      console.log(`Raw data - Bump: ${bump}`);
      console.log(`Raw data - Bitmap data length: ${bitmapDataLength} bytes`);
      console.log(`Raw data - Max trackable slots: ${bitmapDataLength * 8}`);
      console.log(`Raw data - Header size: 16 bytes`);
      console.log(`Raw data - Actual account size: ${accountInfo.data.length} bytes`);
      
      // Try to fetch using program account decoder
      try {
        const account = await program.account.sandwichValidators.fetch(pda);
        console.log(`\nProgram decode - Epoch: ${account.epoch}`);
        console.log(`\nProgram decode - Bump: ${account.bump}`);
        console.log(`Program decode - Bitmap data available: ${bitmapDataLength} bytes`);
        console.log(`Program decode - Max trackable slots: ${bitmapDataLength * 8}`);
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