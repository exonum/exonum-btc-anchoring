# Anchoring btc service deployment

## Contents

* [Bitcoind deployment](#bitcoind-node-deployment)
* [Testnet deployment](#testnet-deployment)
* [Production deployment](#production-deployment)
* [Maintenance](#maintenance)

## Bitcoind node deployment

First of all install `bitcoind` via your package manager and ensure that you
use the latest stable version. You may visit official bitcoin [site][bitcoin:install]
for more information about installation.

Then create bitcoind configuration file in according to this [tutorial][bitcoin_wiki:configuration].

For correct work of the service, the `bitcoind` configuration file
should contain the following settings:

```ini
# Run on the test network instead of the real bitcoin network.
# If you want to use main network comment line bellow:
testnet=1
# server=1 tells Bitcoin-Qt and bitcoind to accept JSON-RPC commands.
server=1
# Maintain a full transaction index, used by the getrawtransaction rpc call.
txindex=1

# Bind to given address to listen for JSON-RPC connections.
# Use [host]:port notation for IPv6.
# This option can be specified multiple times (default: bind to all interfaces)
#rpcbind=<addr>
# You must specify rpcuser and rpcpassword to secure the JSON-RPC API
#rpcuser=<username>
#rpcpassword=YourSuperGreatPasswordNumber_DO_NOT_USE_THIS_OR_YOU_WILL_GET_ROBBED_385593
```

These rpc settings will be used by the service.

After creating configuration file, launch `bitcoind` daemon via command:

```shell
bitcoind --daemon
```

Downloading and indexing of the bitcoin blockchain may take a lot of time,
especially for the mainnet.

## Deployment

For now we have no quick "testnet" deployment, but for fast anchoring
demonstration you can use built-in anchoring example, and regular
deployment guide.

```shell
cargo install --example anchoring
```

### Create blockchain with anchoring

After installation you need to generate configuration.
For this you need:

#### Generate template config

At the first stage, one of the participants creates a template of blockchain
consensus configuration and broadcast it to other members.

```shell
anchoring generate-template \
    <Path where save template config> \
    --anchoring-fee <fee in satoshis> \
    --anchoring-network <Network in which anchoring should work (testnet\bitcoin)>
```

#### Generate config for each node

Then each of the participants generates own public and secret
node config files.

```shell
anchoring generate-config \
    <Path to saved template config> \
    <Path where save public node config> \
    <Path where save private node config> \
    --anchoring-host <bitcoind RPC host> \
    [--anchoring-user <bitcoind RPC username>] \
    [--anchoring-password <bitcoind RPC password>] \
    --peer-addr <external node listening address>
```

Each node should broadcast public config part.

#### Finalizing configuration

When the administrator collects all public configs, he can create
final node configuration.

Participants need to send some bitcoins to the anchoring address in order
to enable Bitcoin anchoring.
For this:

* One of the participants generates initial `funding_tx` by init command:

  ```shell
  anchoring finalize
      <Path to saved private node config> \
      <Path where save finalized node config> \
      [--anchoring-create-funding-tx <Create initial funding tx with given amount in satoshis>] \
      --public-configs <Path to node1 public config>\
      [<Path to node2 public config> ...]
  ```

  This command generates configuration of node and returns
  txid of generated `funding_tx`:

* While others should use this `funding_tx`.

  ```shell
  anchoring finalize
      <Path to saved private node config> \
      <Path where save finalized node config> \
      [--anchoring-funding-txid <Txid of the initial funding tx>] \
      --public-configs <Path to node1 public config>\
      [<Path to node2 public config> ...]
  ```

  This is important, because all nodes in the network should start from
  one state. As result `finalize` command generates node configuration file,
  that can be used to launching.

  Which create the configuration of `N` exonum anchoring nodes in destination
  directory using given `bitcoind` by rpc.

  Also in the generated configuration files you may specify public and private
  API addresses according to this [document][exonum:node_api].

  ***Warning!** `Bitcoind` node should have some bitcoin amount greater
  than `<initial_funds>`, since the initial funding transaction will be
  created during the testnet generation. For testnet you may use a
  [`faucet`][bitcoin:faucet] to get some coins.*

### Launching node

Launch all exonum nodes in the testnet. To launch node `m`, execute:

```shell
anchoring run --node-config <destdir>/<m>.toml --db-path <destdir>/db/<m>
```

If you want to see additional information you may specify log level by
environment variable `RUST_LOG="exonum_btc_anchoring=info"`.

## Maintenance

As maintainer, you can change the anchoring [configuration parameters](#change-configuration-parameters).
These actions must be performed by [Exonum configuration service][exonum:configuration_service].
Visit its [tutorial][exonum:configuration_tutorial] for more explanations.

### Change configuration parameters

Variables that you can modify:

* `fee` - the amount of the fee for the anchoring transaction.
* `frequency` - the frequency in exonum blocks with which the generation of
  a new anchoring transactions occurs.
* `utxo_confirmations` - the minimum number of confirmations in bitcoin network
  to consider the anchoring transaction as fully confirmed. Uses for transition
  and initial funding transactions.
* `funding_tx` - the hex representation of current funding transaction.
  Node would use it as input if it did not spent.
* `anchoring_keys` - the list of hex-encoded compressed bitcoin public keys of
  exonum validators that collects into the current anchoring address.

For the `anchoring` example consensus configuration looks like this:

```json
{
    "fee":10000,
    "frequency":1000,
    "funding_tx":"0100000001c13d4c739390c799344fa89fb701add04e5ccaf3d580e4d4379c4b897e3a2266000000006b483045022100ff88211040a8a95a42ca8520749c1b2b4024ce07b3ed1b51da8bb90ef77dbe5d022034b34ef638d23ef0ea532e2c84a8816cb32021112d4bcf1457b4e2c149d1b83f01210250749a68b12a93c2cca6f86a9a9c9ba37f5191e85334c340856209a17cca349afeffffff0240420f000000000017a914180d8e6b0ad7f63177e943752c278294709425bd872908da0b000000001976a914dee9f9433b3f2d24cbd833f83a41e4c1235efa3f88acd6ac1000",
    "utxo_confirmations":4,
    "anchoring_keys":[
        "03aa5ef3f68ad710b1fcc368b2f1855790f4f0c0fd762dbc1d47339c7ffb8fe363",
        "032a360ef29c339964dba55f701728b8faf34c48ce1988ef85229011cc26d0472f",
        "02e3708c15674f626fd127da715638176df238b2f88730b07ed1700fcede872c25"
    ]
}
```

With these variables you can perform the following actions:

* [Add funds to anchoring wallet via funding transaction](#add-funds).
* [Change list of validators](#change-list-of-validators).
* Just change other variables to more convenient.

#### Add funds

Send to anchoring wallet some btc and save raw transaction body hex.
Wait until transaction got enough confirmations. Then replace `funding_tx`
variable by saved hex.

***Note!** If the current anchoring chain [becomes unusable][exonum:anchoring_transferring]
you may start a new chain by adding corresponding funding transaction.*

#### Change list of validators

***Important warning!** This procedure changes the anchoring address.
Exonum node needs to wait until the last anchored transaction gets enough
confirmations. It is caused by impossibility to sign transaction addressed
to old anchoring address by keys from the current configuration. If the last
anchoring transaction does not get enough confirmations before anchoring
address is changed, the following transferring transaction may be lost because
of possible bitcoin forks and transaction malleability.
See this [article][exonum:anchoring_transferring] for details.*

* Make sure that difference between the activation height (`actual_from`) and
  current `Exonum` blockchain height is enough for get sufficient confirmations
  for the latest anchored transaction. Usually 6 hours are enough for this.
  Calculate how many blocks will be taken during this time and add this number
  to the `current_height`.
* If necessary, [generate](#generate-node-keys) a new key pair for anchoring.
* Change list of validators via editing `anchoring_keys` array.
* Initiate the config update procedure.
* Make sure that config update procedure is not delayed. That is, do not delay
  the voting procedure for the new configuration.
* Look at the new address of the anchoring by the anchoring public
  [API][exonum:anchoring_public_api].

***Note!** If transferring transaction has been lost you need to establish a
new anchoring chain by a new funding transaction.*

### Updating anchoring address in config

Each exonum node stores in the local configuration a map for the anchoring
address and its corresponding private key. The address is encoded using
[`base58check`][bitcoin:base58check] encoding and the private key uses
[`WIF`][bitcoin:wif] format.

```ini
[anchoring_service.node.private_keys]
address = "2NCJYWui4LGNZguUw41xBANbcHoKxSVxyzr"
private_key = "cRf74adxyQzJs7V8fHoyrMDazxzCmKAan63Cfhf9i4KL69zRkdS2"
```

Add the line with new address and corresponding private key for it. If node
public key is not changed you must use the old key for the new address
otherwise use a new key. After modifying the configuration file you need to
restart the node for the changes to take effect.

[bitcoin:install]: https://bitcoin.org/en/full-node#what-is-a-full-node
[bitcoin:faucet]: https://testnet.manu.backend.hamburg/faucet
[bitcoin:base58check]: https://en.bitcoin.it/wiki/Base58Check_encoding
[bitcoin:wif]: https://en.bitcoin.it/wiki/Wallet_import_format
[bitcoin_wiki:configuration]: https://en.bitcoin.it/wiki/Running_Bitcoin#Bitcoin.conf_Configuration_File
[exonum:node_api]: https://github.com/exonum/exonum-doc/blob/master/src/architecture/configuration.md#nodeapi
[exonum:configuration_service]: https://github.com/exonum/exonum-configuration
[exonum:configuration_tutorial]: https://github.com/exonum/exonum-configuration/blob/master/doc/testnet-api-tutorial.md
[exonum:dashboard]: https://github.com/exonum/exonum-dashboard
[exonum:anchoring_transferring]: https://github.com/exonum/exonum-doc/blob/master/src/advanced/bitcoin-anchoring.md#changing-validators-list
[exonum:anchoring_public_api]: https://github.com/exonum/exonum-doc/blob/master/src/advanced/bitcoin-anchoring.md#following-address
