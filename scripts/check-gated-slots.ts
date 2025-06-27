import * as anchor from "@coral-xyz/anchor";
import { web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

async function main() {
  console.log("=== Check Gated Slots ===\n");

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

    // Get current slot info
    const currentSlot = await connection.getSlot();
    const currentEpoch = Math.floor(currentSlot / SLOTS_PER_EPOCH);
    const epochStartSlot = currentEpoch * SLOTS_PER_EPOCH;
    const currentSlotOffset = currentSlot - epochStartSlot;
    
    console.log(`Current slot: ${currentSlot}`);
    console.log(`Current epoch: ${currentEpoch}`);
    console.log(`Slot offset: ${currentSlotOffset}`);

    // Check current PDA state
    const { pda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
    const accountInfo = await connection.getAccountInfo(pda);
    
    if (accountInfo) {
      const bitmapLen = accountInfo.data.readUInt32LE(10);
      console.log(`\nBitmap length: ${bitmapLen} bytes`);
      console.log(`Max trackable slots: ${bitmapLen * 8}`);
      
      // Check the range of slots that should be gated (274094 to 274104)
      const startOffset = Math.max(0, currentSlotOffset - 5);
      const endOffset = Math.min(currentSlotOffset + 5, SLOTS_PER_EPOCH - 1);
      
      console.log(`\nChecking gated range: offset ${startOffset} to ${endOffset}`);
      
      for (let offset = startOffset; offset <= endOffset; offset++) {
        const byteIndex = Math.floor(offset / 8);
        const bitIndex = offset % 8;
        const bytePos = 15 + byteIndex; // header size = 15
        
        if (bytePos < accountInfo.data.length) {
          const byte = accountInfo.data[bytePos];
          const bitSet = (byte >> bitIndex) & 1;
          console.log(`  Offset ${offset}: byte ${byteIndex}, bit ${bitIndex} = ${bitSet ? 'GATED' : 'not gated'} (byte=0x${byte.toString(16).padStart(2, '0')})`);
        } else {
          console.log(`  Offset ${offset}: beyond account data`);
        }
      }
    } else {
      console.log("No PDA exists for current epoch");
    }

  } catch (error) {
    console.error(`Error: ${error}`);
  }
}

main().catch(console.error);