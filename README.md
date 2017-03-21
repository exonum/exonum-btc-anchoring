# Exonum anchoring service
This crate is a part of exonum blockchain. Here will short project description, full specification is situated [here](http://exonum.com/doc/anchoring-spec).

# Build steps
You can see in [exonum](#) crate.

# Bitcoin full node deploy
The anchoring service depends on bitcoind. For correct working of the service you need to launch bitcoind with specific configuration.

## Configuration
Here the sample bitcoin.conf file.
```ini
# Run on the test network instead of the real bitcoin network.
testnet=1
# server=1 tells Bitcoin-Qt and bitcoind to accept JSON-RPC commands
server=1
# Maintain a full transaction index, used by the getrawtransaction rpc call
txindex=1 # для того, чтобы нода индексировала все транзакции 

# Bind to given address to listen for JSON-RPC connections. Use [host]:port notation for IPv6.
# This option can be specified multiple times (default: bind to all interfaces)
#rpcbind=<addr>

# You must set rpcuser and rpcpassword to secure the JSON-RPC api
#rpcuser=<username>
#rpcpassword=YourSuperGreatPasswordNumber_DO_NOT_USE_THIS_OR_YOU_WILL_GET_ROBBED_385593
```
Detailed documentation you can find [here](https://en.bitcoin.it/wiki/Running_Bitcoin#Bitcoin.conf_Configuration_File).

## Launching
You can start the node with the command
```
$ bitcoind --reindex --daemon
```
*note 1: Be sure to wait for the full load of the bitcoin blockchain.*

*note 2: If node deploy for exists configuration be sure that current anchoring address is imported by `importaddress` rpc call.*

# Anchoring testnet example
For quick anchoring demonstration you can install built-in anchoring example.
```
$ cargo install --example anchoring
```

## Generate testnet config
After installation you need to generate testnet configuration
```
$ anchoring generate \
    --output-dir <destdir> <n> \
    --anchoring-host <bitcoin full node host> \
    --anchoring-user <username> \
    --anchoring-password <password> \
    --anchoring-funds <initial funds>
```
Which create the configuration of N nodes using given `bitcoind`.

*warning! It is important that the full node have some bitcoin amount greater  than `<initial_finds>, since the initial funding transaction will create during the testnet generation.*

## Launching testnet
You need to launch the whole testnet nodes. 
The command to launch 'm' node look such this:
```
anchoring run --node-config <destdir>/<m>.toml --leveldb-path <destdir>/db/<m>
```
In addition you may to set http port for configuration update service. More information you can find by invoke `anchoring help`

**Important warning! Do not use this example in production. Secret keys are stored in the single directory on the single machine and can be stolen.*

# Next steps
You can learn the reference [documentation](link) or full [specification](http://exonum.com/doc/anchoring-spec).