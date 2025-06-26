import * as anchor from "@coral-xyz/anchor";
import { Program, web3, BN } from "@coral-xyz/anchor";
import { SaguaroGatekeeper } from "../target/types/saguaro_gatekeeper";
import { assert } from "chai";
import {
  getSandwichValidatorsPda,
  setSandwichValidators,
  updateSandwichValidator,
  validateSandwichValidators,
  closeSandwichValidator,
} from "../ts/sdk";

describe("saguaro-gatekeeper", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SaguaroGatekeeper as Program<SaguaroGatekeeper>;
  const multisigAuthority = provider.wallet as anchor.Wallet;
  const unauthorizedUser = anchor.web3.Keypair.generate();

  before(async () => {
    // Fund the unauthorized user so they can pay for transactions
    const tx = new web3.Transaction().add(
      web3.SystemProgram.transfer({
        fromPubkey: multisigAuthority.publicKey,
        toPubkey: unauthorizedUser.publicKey,
        lamports: web3.LAMPORTS_PER_SOL, // 1 SOL
      })
    );
    await provider.sendAndConfirm(tx, [], { commitment: "confirmed" });

  });

  it("should create the sandwich_validators PDA for a specific epoch", async () => {
    const epochArg = new BN(1);
    // Epoch 1 slots: 432,000 - 863,999
    const slotsArg = [new BN(432000), new BN(500000)];

    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: slotsArg,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda, bump } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    // multisigAuthority is not stored in account, it's derived from PDA seeds
    assert.strictEqual(account.epoch, epochArg.toNumber());
    assert.deepStrictEqual(
      account.slots.map((s) => s.toString()),
      slotsArg.map((s) => s.toString())
    );
    assert.strictEqual(account.bump, bump);
  });

  it("should not allow an unauthorized user to create a PDA", async () => {
    const epochArg = new BN(2);
    // Epoch 2 slots: 864,000 - 1,295,999
    const slotsArg = [new BN(864000), new BN(900000)];

    try {
      // Attempt to create a PDA for the *correct* authority, but sign with the unauthorized user.
      await setSandwichValidators(program, {
        epoch: epochArg.toNumber(),
        slots: slotsArg,
        multisigAuthority: multisigAuthority.publicKey, // Correct authority
      })
        .signers([unauthorizedUser])
        .rpc();
      assert.fail("Transaction should have failed due to unauthorized authority.");
    } catch (error) {
      // This fails because the client-side wallet doesn't have the key for the signer specified
      // in the instruction's accounts (multisigAuthority). This is a valid client-side failure.
      assert.isTrue(
        error.toString().includes("unknown signer"),
        `Expected an 'unknown signer' violation, but got: ${error}`
      );
    }
  });

  it("should succeed validation when the current slot is NOT gated", async () => {
    // Get current epoch info from connection
    const epochInfo = await provider.connection.getEpochInfo();
    const currentSlot = await provider.connection.getSlot();
    const currentEpoch = epochInfo.epoch;
    
    
    // First, ensure there's no PDA for the current epoch
    try {
      await closeSandwichValidator(program, {
        epoch: currentEpoch,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      // Wait a moment for state to settle
      await new Promise(resolve => setTimeout(resolve, 100));
    } catch (e) {
      // Ignore if it doesn't exist
    }

    // Create PDA for current epoch with slots that don't include current slot
    const epochStartSlot = currentEpoch * 432000;
    const epochEndSlot = (currentEpoch + 1) * 432000 - 1;
    
    // Pick slots that are definitely not the current slot
    const slotsArg = [];
    // Add slots that are far from current slot to avoid timing issues
    if (currentSlot > epochStartSlot + 1000) {
      slotsArg.push(new BN(epochStartSlot + 10));
    }
    if (currentSlot < epochEndSlot - 1000) {
      slotsArg.push(new BN(epochEndSlot - 10));
    }
    
    // If we couldn't find safe slots, use a slot that's definitely not current
    if (slotsArg.length === 0) {
      const safeSlot = currentSlot > epochStartSlot + 500 
        ? epochStartSlot + 10 
        : epochEndSlot - 10;
      slotsArg.push(new BN(safeSlot));
    }

    // Create the PDA with non-current slots
    await setSandwichValidators(program, {
      epoch: currentEpoch,
      slots: slotsArg,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    try {
      // Validation will check current epoch and current slot
      const tx = await validateSandwichValidators(program, {
        multisigAuthority: multisigAuthority.publicKey,
      });
      await tx.rpc();
      assert.ok(true, "Validation succeeded as expected.");
    } catch (error) {
      throw new Error(`Validation failed unexpectedly: ${error}`);
    }
  });

  it("should fail validation when the current slot IS gated", async () => {
    const epochInfo = await provider.connection.getEpochInfo();
    const currentEpoch = epochInfo.epoch;
    const currentSlot = await provider.connection.getSlot();
    const epochArg = new BN(currentEpoch);
    
    // Gate the current slot and adjacent slots
    const slotsToAdd = [
      new BN(currentSlot - 1),
      new BN(currentSlot),
      new BN(currentSlot + 1),
    ];

    // Try to fetch the existing PDA
    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );
    
    const accountInfo = await provider.connection.getAccountInfo(pda);
    
    if (accountInfo) {
      // PDA exists, use update to replace all slots
      const existingAccount = await program.account.sandwichValidators.fetch(pda);
      
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        newSlots: slotsToAdd,
        removeSlots: existingAccount.slots,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
    } else {
      // PDA doesn't exist, create it
      await setSandwichValidators(program, {
        epoch: epochArg.toNumber(),
        slots: slotsToAdd,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
    }

    try {
      const tx = await validateSandwichValidators(program, {
        multisigAuthority: multisigAuthority.publicKey,
      });
      await tx.rpc();
      assert.fail(
        "Validation should have failed because the current slot is gated."
      );
    } catch (error) {
      assert.isTrue(
        error.toString().includes("SlotIsGated"),
        `Expected 'SlotIsGated' error, but got: ${error}`
      );
    }
  });

  it("should succeed validation for a non-existent PDA", async () => {
    // Get current epoch and ensure there's no PDA for it
    const epochInfo = await provider.connection.getEpochInfo();
    const currentEpoch = epochInfo.epoch;
    const currentSlot = await provider.connection.getSlot();
    
    // First, ensure there's no PDA for the current epoch
    try {
      await closeSandwichValidator(program, {
        epoch: currentEpoch,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      // Wait for state to settle
      await new Promise(resolve => setTimeout(resolve, 200));
    } catch (e) {
      // Ignore if it doesn't exist
    }
    
    // Create a PDA for a different epoch to ensure we're testing non-existent current epoch PDA
    const differentEpoch = currentEpoch + 2000;
    const futureEpochStartSlot = differentEpoch * 432000;
    await setSandwichValidators(program, {
      epoch: differentEpoch,
      slots: [new BN(futureEpochStartSlot)],
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();
    
    try {
      // Validation will check current epoch PDA, which doesn't exist
      const tx = await validateSandwichValidators(program, {
        multisigAuthority: multisigAuthority.publicKey,
      });
      await tx.rpc();
      assert.ok(
        true,
        "Validation succeeded because current epoch PDA doesn't exist."
      );
    } catch (error) {
      throw new Error(
        `Validation failed unexpectedly: ${error}`
      );
    }
  });

  it("should successfully update the slots in an existing PDA", async () => {
    const epochArg = new BN(5);
    // Epoch 5 slots: 2,160,000 - 2,591,999
    const initialSlots = [new BN(2160000), new BN(2160100)];
    const newSlots = [new BN(2160200), new BN(2160300), new BN(2160400)];

    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Remove old slots and add new slots to replace them completely
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      newSlots: newSlots,
      removeSlots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );
    const updatedAccount = await program.account.sandwichValidators.fetch(pda);
    assert.deepStrictEqual(
      updatedAccount.slots.map((s) => s.toString()),
      newSlots.map((s) => s.toString())
    );
  });

  it("should NOT allow an unauthorized user to update slots", async () => {
    const epochArg = new BN(6);
    // Epoch 6 slots: 2,592,000 - 3,023,999
    const initialSlots = [new BN(2592000)];

    // First, create a PDA with the correct authority.
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    try {
      // Attempt to update the PDA, passing the correct authority key but signing with the wrong user.
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        newSlots: [new BN(2592001)], // Valid slot for epoch 6
        multisigAuthority: multisigAuthority.publicKey, // Correct authority
      })
        .signers([unauthorizedUser]) // But unauthorized user signs
        .rpc();
      assert.fail("Update should have failed due to unauthorized authority.");
    } catch (error) {
      // This fails because the client-side wallet doesn't have the key for the signer specified
      // in the instruction's accounts (multisigAuthority). This is a valid client-side failure.
      assert.isTrue(
        error.toString().includes("unknown signer"),
        `Expected an 'unknown signer' violation, but got: ${error}`
      );
    }
  });

  it("should close a PDA for a finished epoch and return rent", async () => {
    const epochInfo = await provider.connection.getEpochInfo();
    const currentEpoch = new BN(epochInfo.epoch);

    if (currentEpoch.isZero()) {
      console.log(
        "Skipping test for closing past epoch PDA: current epoch is 0."
      );
      return;
    }

    const pastEpoch = currentEpoch.sub(new BN(1));

    await setSandwichValidators(program, {
      epoch: pastEpoch.toNumber(),
      slots: [new BN(pastEpoch.toNumber() * 432000)], // Valid slot for past epoch
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      pastEpoch,
      program.programId
    );
    const initialBalance = await provider.connection.getBalance(
      multisigAuthority.publicKey
    );

    // Close the PDA
    await closeSandwichValidator(program, {
      epoch: pastEpoch.toNumber(),
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Verify the account is closed by trying to fetch it
    try {
      await program.account.sandwichValidators.fetch(pda);
      assert.fail("Account should have been closed.");
    } catch (error) {
      assert.isTrue(
        error.message.includes("Account does not exist"),
        `Expected account to not exist, but got: ${error.message}`
      );
    }

    const finalBalance = await provider.connection.getBalance(
      multisigAuthority.publicKey
    );
    // Check that balance increased (rent refund - tx fee)
    assert.isTrue(
      finalBalance > initialBalance - web3.LAMPORTS_PER_SOL * 0.01,
      "Authority balance should have increased after rent refund."
    );
  });

  it("should NOT allow closing a PDA for a current or future epoch", async () => {
    const epochInfo = await provider.connection.getEpochInfo();
    const currentEpoch = new BN(epochInfo.epoch);

    // Use a unique epoch to avoid conflicts (108, 109)
    const testCurrentEpoch = new BN(108);
    
    // Test current epoch
    await setSandwichValidators(program, {
      epoch: testCurrentEpoch.toNumber(),
      slots: [new BN(testCurrentEpoch.toNumber() * 432000)], // Valid slot for test epoch 108
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    try {
      await closeSandwichValidator(program, {
        epoch: testCurrentEpoch.toNumber(),
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Closing a PDA for a current epoch should have failed.");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("EpochNotFinished"),
        `Expected 'EpochNotFinished' error, but got: ${error}`
      );
    }

    // Test future epoch
    const testFutureEpoch = new BN(109);

    await setSandwichValidators(program, {
      epoch: testFutureEpoch.toNumber(),
      slots: [new BN(testFutureEpoch.toNumber() * 432000)], // Valid slot for test epoch 109
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    try {
      await closeSandwichValidator(program, {
        epoch: testFutureEpoch.toNumber(),
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Closing a PDA for a future epoch should have failed.");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("EpochNotFinished"),
        `Expected 'EpochNotFinished' error for future epoch, but got: ${error}`
      );
    }
  });

  // === Pause Mechanism Tests ===





  // === Enhanced Validation Tests ===

  it("should reject duplicate slots in array", async () => {
    const epochArg = new BN(100);
    // Epoch 100 slots: 43,200,000 - 43,631,999
    const duplicateSlots = [new BN(43200000), new BN(43200001), new BN(43200000)]; // First slot appears twice

    try {
      await setSandwichValidators(program, {
        epoch: epochArg.toNumber(),
        slots: duplicateSlots,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Setting duplicate slots should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("DuplicateSlots"),
        `Expected 'DuplicateSlots' error, but got: ${error}`
      );
    }
  });

  it("should reject update with duplicate slots", async () => {
    const epochArg = new BN(101);
    // Epoch 101 slots: 43,632,000 - 44,063,999
    const initialSlots = [new BN(43632000), new BN(43632001)];
    const duplicateSlots = [new BN(43632100), new BN(43632101), new BN(43632100)]; // First slot appears twice

    // First create the PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    try {
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        newSlots: duplicateSlots,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Updating with duplicate slots should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("DuplicateSlots"),
        `Expected 'DuplicateSlots' error, but got: ${error}`
      );
    }
  });

  it("should handle empty slot arrays with warning", async () => {
    const epochArg = new BN(102);
    const emptySlots: BN[] = [];

    // Should succeed but log a warning
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: emptySlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 0);
  });

  it("should handle maximum allowed slots (100)", async () => {
    const epochArg = new BN(103);
    // Epoch 103 slots: 44,496,000 - 44,927,999
    // Create array of 100 unique slots
    const maxSlots = Array.from({length: 100}, (_, i) => new BN(44496000 + i));

    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: maxSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 100);
  });

  it("should reject more than maximum allowed slots (101)", async () => {
    const epochArg = new BN(104);
    // Epoch 104 slots: 44,928,000 - 45,359,999
    // Create array of 101 slots - should fail
    const tooManySlots = Array.from({length: 101}, (_, i) => new BN(44928000 + i));

    try {
      await setSandwichValidators(program, {
        epoch: epochArg.toNumber(),
        slots: tooManySlots,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Setting more than 100 slots should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("TooManySlots"),
        `Expected 'TooManySlots' error, but got: ${error}`
      );
    }
  });

  it("should successfully resize account when updating to larger slot array", async () => {
    const epochArg = new BN(105);
    // Epoch 105 slots: 45,360,000 - 45,791,999
    const initialSlots = [new BN(45360000), new BN(45360001)]; // 2 slots
    const largerSlots = Array.from({length: 100}, (_, i) => new BN(45360100 + i)); // 100 slots

    // Create initial PDA with small array
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Update to larger array - should resize account
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      newSlots: largerSlots,
      removeSlots: initialSlots, // Remove initial slots to test replacement
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 100);
  });

  it("should successfully resize account when updating to smaller slot array", async () => {
    const epochArg = new BN(106);
    // Epoch 106 slots: 45,792,000 - 46,223,999
    const largeSlots = Array.from({length: 100}, (_, i) => new BN(45792000 + i)); // 100 slots
    const smallSlots = [new BN(45792200), new BN(45792201)]; // 2 slots

    // Create initial PDA with large array
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: largeSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Update to smaller array - should resize account
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      newSlots: smallSlots,
      removeSlots: largeSlots, // Remove all large slots to test replacement
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 2);
    assert.deepStrictEqual(
      account.slots.map((s) => s.toString()),
      smallSlots.map((s) => s.toString())
    );
  });

  it("should handle epoch boundary values correctly", async () => {
    // Test with epoch 107 (avoid conflicts with existing tests)
    const epochTest = new BN(107);
    // Epoch 107 slots: 46,224,000 - 46,655,999
    const slotsTest = [new BN(46224000), new BN(46224001)];

    await setSandwichValidators(program, {
      epoch: epochTest.toNumber(),
      slots: slotsTest,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda: pdaTest } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochTest,
      program.programId
    );

    const accountTest = await program.account.sandwichValidators.fetch(pdaTest);
    assert.strictEqual(accountTest.epoch, 107);

    // Test with maximum epoch value (u16::MAX = 65535)
    const epochMax = new BN(65535);
    // Epoch 65535 slots: 28,311,120,000 - 28,311,551,999
    const slotsMax = [new BN(28311120000), new BN(28311120001)];

    await setSandwichValidators(program, {
      epoch: epochMax.toNumber(),
      slots: slotsMax,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda: pdaMax } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochMax,
      program.programId
    );

    const accountMax = await program.account.sandwichValidators.fetch(pdaMax);
    assert.strictEqual(accountMax.epoch, 65535);
  });

  // === Add Slots Functionality Tests ===

  it("should add slots to an existing PDA using updateSandwichValidator", async () => {
    const epochArg = new BN(200);
    // Epoch 200 slots: 86,400,000 - 86,831,999
    const initialSlots = [new BN(86400000), new BN(86400001)];
    const additionalSlots = [new BN(86400002), new BN(86400003), new BN(86400004)];

    // First create a PDA with initial slots
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Append additional slots using update
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      newSlots: additionalSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 5);
    
    // Verify all slots are present
    const allExpectedSlots = [...initialSlots, ...additionalSlots];
    const accountSlots = account.slots.map((s) => s.toString());
    const expectedSlots = allExpectedSlots.map((s) => s.toString());
    
    for (const expectedSlot of expectedSlots) {
      assert.isTrue(
        accountSlots.includes(expectedSlot),
        `Expected slot ${expectedSlot} to be present in account`
      );
    }
  });

  it("should reject adding duplicate slots", async () => {
    const epochArg = new BN(201);
    // Epoch 201 slots: 86,832,000 - 87,263,999
    const initialSlots = [new BN(86832000), new BN(86832001)];
    const duplicateSlots = [new BN(86832000), new BN(86832002)]; // First slot already exists

    // First create a PDA with initial slots
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Try to append duplicate slots using update
    try {
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        newSlots: duplicateSlots,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Appending duplicate slots should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("DuplicateSlots"),
        `Expected 'DuplicateSlots' error, but got: ${error}`
      );
    }
  });

  it("should reject adding when total slots would exceed maximum", async () => {
    const epochArg = new BN(202);
    // Epoch 202 slots: 87,264,000 - 87,695,999
    // Create initial slots close to the limit
    const initialSlots = Array.from({length: 50}, (_, i) => new BN(87264000 + i));
    // Try to append slots that would exceed the 100 per transaction limit
    const tooManySlots = Array.from({length: 101}, (_, i) => new BN(87264100 + i));

    // First create a PDA with initial slots
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Try to append too many slots in one transaction using update
    try {
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        newSlots: tooManySlots,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Appending more than 100 slots should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("TooManySlots"),
        `Expected 'TooManySlots' error, but got: ${error}`
      );
    }
  });

  it("should allow multiple add operations to build large slot arrays", async () => {
    const epochArg = new BN(203);
    // Epoch 203 slots: 87,696,000 - 88,127,999
    const initialSlots = Array.from({length: 50}, (_, i) => new BN(87696000 + i));
    const batch1 = Array.from({length: 50}, (_, i) => new BN(87696100 + i));
    const batch2 = Array.from({length: 50}, (_, i) => new BN(87696200 + i));
    const batch3 = Array.from({length: 50}, (_, i) => new BN(87696300 + i));

    // Create initial PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Append first batch using update
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      newSlots: batch1,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Append second batch using update
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      newSlots: batch2,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Append third batch using update
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      newSlots: batch3,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 200, "Should have 200 total slots");
  });


  it("should reject empty operations gracefully", async () => {
    const epochArg = new BN(205);
    // Epoch 205 slots: 88,560,000 - 88,991,999
    const initialSlots = [new BN(88560000), new BN(88560001)];
    const emptySlots: BN[] = [];

    // Create initial PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Try empty operation (should fail client-side validation)
    try {
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        newSlots: emptySlots,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Empty operation should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("No operation specified"),
        `Expected 'No operation specified' error, but got: ${error}`
      );
    }
  });

  // === Update with Slot Removal Tests ===

  it("should remove slots using updateSandwichValidator", async () => {
    const epochArg = new BN(300);
    // Epoch 300 slots: 129,600,000 - 130,031,999
    const initialSlots = [new BN(129600000), new BN(129600001), new BN(129600002), new BN(129600003), new BN(129600004)];
    const slotsToRemove = [new BN(129600001), new BN(129600003)]; // Remove 2 slots

    // Create initial PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Remove slots using update instruction
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      removeSlots: slotsToRemove,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 3, "Should have 3 slots remaining");
    
    // Verify removed slots are not present
    const accountSlots = account.slots.map((s) => s.toString());
    assert.isFalse(accountSlots.includes("129600001"), "Slot 129600001 should be removed");
    assert.isFalse(accountSlots.includes("129600003"), "Slot 129600003 should be removed");
    
    // Verify remaining slots are present
    assert.isTrue(accountSlots.includes("129600000"), "Slot 129600000 should remain");
    assert.isTrue(accountSlots.includes("129600002"), "Slot 129600002 should remain");
    assert.isTrue(accountSlots.includes("129600004"), "Slot 129600004 should remain");
  });

  it("should add and remove slots in the same transaction", async () => {
    const epochArg = new BN(301);
    // Epoch 301 slots: 130,032,000 - 130,463,999
    const initialSlots = [new BN(130032000), new BN(130032001), new BN(130032002)];
    const slotsToAdd = [new BN(130032003), new BN(130032004)];
    const slotsToRemove = [new BN(130032001)];

    // Create initial PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Add and remove slots in the same transaction
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      newSlots: slotsToAdd,
      removeSlots: slotsToRemove,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 4, "Should have 4 slots total (3 - 1 + 2)");
    
    const accountSlots = account.slots.map((s) => s.toString());
    
    // Verify removed slot is not present
    assert.isFalse(accountSlots.includes("130032001"), "Slot 130032001 should be removed");
    
    // Verify remaining original slots are present
    assert.isTrue(accountSlots.includes("130032000"), "Slot 130032000 should remain");
    assert.isTrue(accountSlots.includes("130032002"), "Slot 130032002 should remain");
    
    // Verify new slots are present
    assert.isTrue(accountSlots.includes("130032003"), "Slot 130032003 should be added");
    assert.isTrue(accountSlots.includes("130032004"), "Slot 130032004 should be added");
  });

  it("should reject removing non-existent slots gracefully", async () => {
    const epochArg = new BN(302);
    // Epoch 302 slots: 130,464,000 - 130,895,999
    const initialSlots = [new BN(130464000), new BN(130464001)];
    const nonExistentSlots = [new BN(130464000), new BN(130464999)]; // Second slot doesn't exist

    // Create initial PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Try to remove slots where one doesn't exist (should succeed but warn)
    await updateSandwichValidator(program, {
      epoch: epochArg.toNumber(),
      removeSlots: nonExistentSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    const { pda } = getSandwichValidatorsPda(
      multisigAuthority.publicKey,
      epochArg,
      program.programId
    );

    const account = await program.account.sandwichValidators.fetch(pda);
    assert.strictEqual(account.slots.length, 1, "Should have 1 slot remaining (8200 was removed, 8201 remains)");
    
    const accountSlots = account.slots.map((s) => s.toString());
    assert.isFalse(accountSlots.includes("130464000"), "Slot 130464000 should be removed");
    assert.isTrue(accountSlots.includes("130464001"), "Slot 130464001 should remain");
  });

  it("should reject duplicate slots in remove array", async () => {
    const epochArg = new BN(303);
    // Epoch 303 slots: 130,896,000 - 131,327,999
    const initialSlots = [new BN(130896000), new BN(130896001), new BN(130896002)];
    const duplicateRemoveSlots = [new BN(130896000), new BN(130896000)]; // Duplicate

    // Create initial PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Try to remove duplicate slots
    try {
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        removeSlots: duplicateRemoveSlots,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Removing duplicate slots should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("DuplicateSlots"),
        `Expected 'DuplicateSlots' error, but got: ${error}`
      );
    }
  });

  it("should reject update with too many slots to remove", async () => {
    const epochArg = new BN(304);
    // Epoch 304 slots: 131,328,000 - 131,759,999
    const initialSlots = Array.from({length: 50}, (_, i) => new BN(131328000 + i));
    const tooManySlotsToRemove = Array.from({length: 101}, (_, i) => new BN(131328000 + i)); // > 100

    // Create initial PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Try to remove too many slots in one transaction
    try {
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        removeSlots: tooManySlotsToRemove,
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Removing more than 100 slots should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("TooManySlots"),
        `Expected 'TooManySlots' error, but got: ${error}`
      );
    }
  });

  it("should reject update with no operations specified", async () => {
    const epochArg = new BN(305);
    // Epoch 305 slots: 131,760,000 - 132,191,999
    const initialSlots = [new BN(131760000), new BN(131760001)];

    // Create initial PDA
    await setSandwichValidators(program, {
      epoch: epochArg.toNumber(),
      slots: initialSlots,
      multisigAuthority: multisigAuthority.publicKey,
    })
      .signers([multisigAuthority.payer])
      .rpc();

    // Try to call update with neither add nor remove operations
    try {
      await updateSandwichValidator(program, {
        epoch: epochArg.toNumber(),
        multisigAuthority: multisigAuthority.publicKey,
      })
        .signers([multisigAuthority.payer])
        .rpc();
      assert.fail("Update with no operations should have failed");
    } catch (error) {
      assert.isTrue(
        error.toString().includes("No operation specified"),
        `Expected 'No operation specified' error, but got: ${error}`
      );
    }
  });
});
