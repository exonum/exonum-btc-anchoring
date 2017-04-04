# Complete tutorial

*Important warning! Do not use this example in production. Secret keys are stored in the single directory on the single machine and can be stolen. 
You need to provide secure exchange procedure for public part of blockchain configuration (e.g validators keys, btc public keys, configuration of consensus, etc...).*

## Contents
* [Bitcoind deploy](#bitcoind-deploy)
* [Launch testnet](#launch-testnet)
* [Testnet maintaining](#testnet-maintaining)
* [Funding testnet](#funding-testnet)
* [Change testnet keys](#change-testnet-keys)

## Bitcoind deploy

### Configuration
Create `bitcoin.conf` file like this: 
```ini
# Run on the test network instead of the real bitcoin network.
testnet=1
# server=1 tells Bitcoin-Qt and bitcoind to accept JSON-RPC commands
server=1
# Maintain a full transaction index, used by the getrawtransaction rpc call
txindex=1

# Bind to given address to listen for JSON-RPC connections. Use [host]:port notation for IPv6.
# This option can be specified multiple times (default: bind to all interfaces)
#rpcbind=<addr>

# You must set rpcuser and rpcpassword to secure the JSON-RPC api
#rpcuser=<username>
#rpcpassword=YourSuperGreatPasswordNumber_DO_NOT_USE_THIS_OR_YOU_WILL_GET_ROBBED_385593
```
Detailed documentation you can find [here](https://en.bitcoin.it/wiki/Running_Bitcoin#Bitcoin.conf_Configuration_File).

### Launching
Start the node with the command.
```
$ bitcoind --reindex --daemon
```
*note 1: Be sure to wait for the full load of the bitcoin blockchain.*

*note 2: If node deploy for exists configuration be sure that current anchoring address is imported by `importaddress` rpc call.*

## Launch testnet
For quick anchoring demonstration you can install built-in anchoring example.
```
$ cargo install --example anchoring
```

### Generate testnet config
After installation you need to generate testnet configuration.
```
$ anchoring generate \
    --output-dir <destdir> <n> \
    --anchoring-host <bitcoin full node host> \
    --anchoring-user <username> \
    --anchoring-password <password> \
    --anchoring-funds <initial funds>
```
Which create the configuration of N nodes using given `bitcoind`.

*warning! It is important that the full node have some bitcoin amount greater  than `<initial_finds>`, since the initial funding transaction will create during the testnet generation.*

### Launching testnet
You need to launch the whole testnet nodes. 
The command to launch `m` node look such this:
```
$ anchoring run --node-config <destdir>/<m>.toml --leveldb-path <destdir>/db/<m>
```
In addition you may to set http port for configuration update service. More information you can find by invoke `anchoring help`

If you want to see additional information including current testnet `multisig` address you may set environment variable `RUST_LOG="anchoring_btc_service=info"`.

## Testnet maintaining
You may be need to change fee, add funds, change keys, etc... These actions must be made via 
[Exonum configuration service](https://github.com/exonum/exonum-configuration). 

For the `anchoring` example consensus configuration looks like this:
```json
{
    "actual_from":35000,
    "validators":[
        "00ecddc2d0ff44192ae249f1ebfc25e996e611258b903760b15649b5a8f7c1a8", 
        "6edeb1940d45d8a0cc31e14b6e8c0e5792ada7db6f5cb24a649d6e5db751f260", 
        "d46320637a9bf04669af9b14d4164b447a7a0af47077dae177bfcdfbb6961df4"
    ],
    "consensus":{
        "round_timeout":5000,
        "status_timeout":10000,
        "peers_timeout":15000,
        "propose_timeout":50,
        "txs_block_limit":1000
    },
    "services":{
        "1":null,
        "3":{
            "fee":10000,
            "frequency":1000,
            "funding_tx":"0100000001c13d4c739390c799344fa89fb701add04e5ccaf3d580e4d4379c4b897e3a2266000000006b483045022100ff88211040a8a95a42ca8520749c1b2b4024ce07b3ed1b51da8bb90ef77dbe5d022034b34ef638d23ef0ea532e2c84a8816cb32021112d4bcf1457b4e2c149d1b83f01210250749a68b12a93c2cca6f86a9a9c9ba37f5191e85334c340856209a17cca349afeffffff0240420f000000000017a914180d8e6b0ad7f63177e943752c278294709425bd872908da0b000000001976a914dee9f9433b3f2d24cbd833f83a41e4c1235efa3f88acd6ac1000",
            "utxo_confirmations":4,
            "validators":[
                "03aa5ef3f68ad710b1fcc368b2f1855790f4f0c0fd762dbc1d47339c7ffb8fe363", 
                "032a360ef29c339964dba55f701728b8faf34c48ce1988ef85229011cc26d0472f",
                "02e3708c15674f626fd127da715638176df238b2f88730b07ed1700fcede872c25"
            ]
        }
    }
}
```
You can modify any variable in the configuration and later initiate the procedure for making changes with other validators.

## Funding testnet
Sometimes you need to add additional funds to anchoring. 
This is done in two stages:
* Send funds to current `multisig` address and save it hex.
* Change `funding_tx` variable via `Exonum Configuration Service` to stored hex. 
* Initiate the config update procedure.

## Change other variables like `fee` or `utxo_confirmations`
Just change interesting variable and initiate the config update procedure.

## Change testnet keys
*Important warning! This procedure changes the `multisig` address and node needs to wait 
until the last anchored transaction get enough confirmations.
It happens because unable to sign transaction addressed to old `multisig` address by keys 
from current configuration. And we needs to be sure that the `transfering transaction` do not lost in any cases!
See this [article](#todo) for details.*

* Make sure that difference between `actual_from` and `current_height` is enough for get sufficient confirmations for `latest anchored transaction`. Usually enough 6 hours for this, calculate how many blocks will be taken during this time and add this number to the `current_height`.
* Initiate the config update procedure.
* Make sure that config update procedure do not delayed.

*If `transferring transaction` have been lost you need to establish a new anchoring chain by a new `funding transaction`.*