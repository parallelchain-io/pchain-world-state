# World State

Following in Ethereum, we call the user-visible state that Mainnet maintains the “World State”. The world state is a set of key-value tuples representing the state of every “Account”, stored inside a paritytech/trie-db Merkle Patricia Trie (MPT). 

## Account

An account can be thought of abstractly as an agent that can trigger state changes. Each account has 5 state variables, each stored in a specific single byte key suffix.

| Preflix | Field | Description |
|:---|:---|:---|
|0x00| None | An u64 number. For an External Account, the number of Transactions originating from this Account so far on chain. Empty for a Contract Account. |
|0x01| Balance | An u64 number. The number of Grays owned by the Account.|
|0x02| Contract | Arbitrary bytes. For a Contract Account, the Contract’s WASM Bytecode. Empty for an External Account. |
|0x03| CBI Version | An u32 number. for a Contract Account, the version of the Contract Binary Interface that the Contract’s Code expects. Empty for an External Account. |
|0x04| Storage Hash | A 32 bytes hash. | for a Contract Account, the root hash of its Storage Trie. Empty for an External Account. |

Each Contract Account is associated with an MPT called a Storage Trie. A Contract’s Storage can only be mutated from inside Call Transactions, and then only from the specific Contract’s Code.

## Network Account Storage

The world state keeps track of state that have significance to the entire network by an identified Network Account, which resides at a specific address. Network Account maintains network-wide state in the Storage Trie. This Account is not associated with Ed25519 material. The network-significant data that the Network Account stores is composed of various fields stored in its Storage Trie.

The state variables stored in the network account storage trie are:

| Prefix | Field | Type| Description |
|:---|:---|:---|:---|
|0x00 |Previous Validator Set (PVS) | IndexHeap\<[Pool](#pool)\> | The set of pools that form the validator set in the previous epoch. The stake in this validator set is locked until the next epoch to give time for evidence to be published. |
|0x01 |Current Validator Set (VS) | IndexHeap\<[Pool](#pool)\> | The set of  pools that form the validator set in the current epoch. |
|0x02 |Next Validator Set (NVS) | IndexHeap\<[PoolKey](#poolkey)\> | The N Pools with the largest powers. These will become the validator set in the next epoch. If two pools have the same power, the one with the greater operator address will be ordered first. |
|0x03 |Pools | Map of address to [Pool](#pool) | The set of pools that are accepting [Stake](#stake) and currently competing to become part of the next validator set, indexed by the operator of the pool.|
|0x04 |Deposits | Map of address to [Deposit](#deposit) | The locked balance of an account for a particular pool, which determines the limit of voting power that the owner can delegate. |
|0x05 |Current Epoch | u64 | The current epoch number. |
|0x06 |Current Epoch Start View | u64 | The HotStuff-rs view number on which the current epoch started. |
|0x07 |Previous Epoch Start View | u64 | the HotStuff-rs view number of which the previous epoch started. |
|0x08 |Published Evidence | Map of 32-byte hash to bool | The set of CryptoHashes of evidence  that has been previously published. This is used to make sure that the same evidence is not published twice. |

### Pool 

Pool describes the place that stake owners can stake to. It contains variables:
- Operator - the operator of the pool. This account will be proposing blocks and signing votes on the pool’s behalf.
- Power - the sum of the powers of the pool’s operator stake and delegated stakes.
- Commission Rate - the percentage (0-100%) of the epoch’s issuance rewarded to the pool that will go towards the operator’s stake (or their balance, if the operator did not stake to itself).
- Operator Stake - the stake of the operator in its own pool. This may be empty.
- Delegated Stakes - the largest stakes delegated to this pool.

### PoolKey

PoolKey is a small description of a pool. It contains variables:
- Operator - the operator of the pool.
- Power - the sum of the powers of the pool’s operator stake and delegated stakes.

### Deposit

The locked balance of an account for a particular pool. It contains variables:
- Balance - the balance of this deposit.
- Auto Stake Rewards - a setting of Deposit that tells whether the received reward in epoch transaction should be automatically staked to the pool.

### Stake

Stake represents the voting power of an account. It could be a delegated stakes or operation's own state. It contains variables:
- Operator - the operator of the pool.
- Power - the power of this stake to the pool.


## World State Key

World State Key (WSKey) is the key to be stored in a MPT. There are two levels of Trie, WorldState and Storage.

At World State Level, the key is prefixed with Address and a byte called Visibility. The value of 0x00 is used in this case (denoted as `Protected`).

List of WSKeys: (The operator `++` means concatenation.)

|Field | Key |
|:---|:---|
|None | Address ++ Protected ++ 0x00 |
|Balance | Address ++ Protected ++ 0x01 |
|Contract | Address ++ Protected ++ 0x02 |
|CBI Version | Address ++ Protected ++ 0x03 |
|Storage Hash | Address ++ Protected ++ 0x04 |

At Storage Level, the key is prefixed with Address and a byte called Visibility. The value of 0x01 is used in this case (denoted as `Public`).

|Field | Key |
|:---|:---|
|Key in Storage | Address ++ Public ++ AppKey |

AppKey is arbitrary bytes that are defined by Contract. 

In the case of Network Account, its Storage Trie utilizes the AppKey as mentioned in section [Network Account Storage](#network-account-storage). List of Network Account keys:

|Field | **AppKey** |
|:---|:---|
|Index of Pool in PVS| 0x00 ++ 0x01 ++ Operator Address|
|Value of Pool in PVS| 0x00 ++ 0x02 ++ Index (4 LE bytes)|
|Operator of a Pool in PVS| 0x00 ++ 0x03 ++ Operator Address ++ 0x00 |
|Power of a Pool in PVS| 0x00 ++ 0x03 ++ Operator Address ++ 0x01 |
|Commission Rate of a Pool in PVS| 0x00 ++ 0x03 ++ Operator Address ++ 0x02 |
|Operator's Stake of a Pool in PVS| 0x00 ++ 0x03 ++ Operator Address ++ 0x03 |
|Index of the stakes in PVS|0x00 ++ 0x03 ++ Operator Address ++ 0x04 ++ 0x01 ++ Owner Address|
|Value of the stakes in PVS|0x00 ++ 0x03 ++ Operator Address ++ 0x04 ++ 0x02 ++ Index (4 LE bytes) |
|Index of Pool in VS| 0x01 ++ 0x01 ++ Operator Address|
|Value of Pool in VS| 0x01 ++ 0x02 ++ Index (4 LE bytes)|
|Operator of a Pool in VS| 0x01 ++ 0x03 ++ Operator Address ++ 0x00 |
|Power of a Pool in VS| 0x01 ++ 0x03 ++ Operator Address ++ 0x01 |
|Commission Rate of a Pool in VS| 0x01 ++ 0x03 ++ Operator Address ++ 0x02 |
|Operator's Stake of a Pool in VS| 0x01 ++ 0x03 ++ Operator Address ++ 0x03 |
|Index of the stakes in VS|0x01 ++ 0x03 ++ Operator Address ++ 0x04 ++ 0x01 ++ Owner Address|
|Value of the stakes in VS|0x01 ++ 0x03 ++ Operator Address ++ 0x04 ++ 0x02 ++ Index (4 LE bytes) |
|Pool Operator|0x03 ++ Operator Address ++ 0x00|
|Pool Power|0x03 ++ Operator Address ++ 0x01|
|Commission rate|0x03 ++ Operator Address ++ 0x02|
|Operator's Own Stake|0x03 ++ Operator Address ++ 0x03|
|Index of Delegated Stakes in Pool|0x03 ++ Operator Address ++ 0x04 ++ 0x01 ++ Owner Address|
|Value of Delegated Stakes in Pool|0x03 ++ Operator Address ++ 0x04 ++ 0x02 ++ Index (4 LE bytes)|
|Deposits Balance|0x04 ++ Operator Address ++ Owner Address ++ 0x00|
|Deposits Auto Stake Rewards|0x04 ++ Operator Address ++ Owner Address ++ 0x01|
|Current Epoch|0x05|
