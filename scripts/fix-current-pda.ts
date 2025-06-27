import * as anchor from "@coral-xyz/anchor";
import { web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import {
  getSandwichValidatorsPda,
  expandSandwichValidators,
  SLOTS_PER_EPOCH,
} from "../ts/sdk";
import { readFileSync } from "fs";

async function main() {
  console.log("=== Fix Current Epoch PDA Bitmap Length ===\n");

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

    // Get current epoch
    const currentSlot = await connection.getSlot();
    const currentEpoch = Math.floor(currentSlot / SLOTS_PER_EPOCH);
    
    console.log(`Current epoch: ${currentEpoch}`);

    // Check current PDA state
    const { pda } = getSandwichValidatorsPda(wallet.publicKey, new BN(currentEpoch), program.programId);
    const beforeInfo = await connection.getAccountInfo(pda);
    
    if (beforeInfo) {
      const beforeBitmapLen = beforeInfo.data.readUInt32LE(10);
      console.log(`Before fix - Account size: ${beforeInfo.data.length} bytes, Bitmap length: ${beforeBitmapLen} bytes`);
      
      // Try to expand the PDA to fix the bitmap length
      console.log("Attempting to fix bitmap length by calling expand...");
      
      await expandSandwichValidators(program, {
        epoch: currentEpoch,
        multisigAuthority: wallet.publicKey,
      })
        .signers([wallet.payer])
        .rpc();

      // Check after expansion
      const afterInfo = await connection.getAccountInfo(pda);
      if (afterInfo) {
        const afterBitmapLen = afterInfo.data.readUInt32LE(10);
        console.log(`After fix - Account size: ${afterInfo.data.length} bytes, Bitmap length: ${afterBitmapLen} bytes`);
        console.log(`Max trackable slots: ${afterBitmapLen * 8}`);
      }
    } else {
      console.log("No PDA exists for current epoch");
    }

  } catch (error) {
    console.error(`Error: ${error}`);
  }
}

main().catch(console.error);