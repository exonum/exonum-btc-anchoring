# A Maintenance Guide

This manual is intended for advanced users who are already able to launch an anchoring
instance in accordance of a [newbie](newbie.md) guide.

The manual describes most common procedures of the service maintenance:

* [Funding of anchoring chain wallet](#Funding-of-anchoring-chain-wallet)
* [Modification of configuration parameters](#Modification-of-configuration-parameters)
* [Changing list of anchoring nodes](#Changing-list-of-anchoring-nodes)

## Funding of anchoring chain wallet

Sometimes you have to replenish the anchoring chain wallet to keep anchoring going.
To do it, send some amount of Bitcoins to the actual anchoring address, which
you can obtain by the public HTTP API [endpoint][anchoring:actual-address].
And then save received transaction hex and wait until it get enough confirmations
in Bitcoin network. When the transaction receives enough confirmations, send it
to each of the anchoring nodes using the corresponding private HTTP API
[endpoint][anchoring:add-funds].

***Beware!** The anchoring node itself does not check that the funding
transaction is confirmed and can be spend. If you send a malformed transaction,
the behaviour of the anchoring node is undefined.*

## Modification of configuration parameters

You can use the [`exonum_launcher`][exonum_launcher] utility to change the
anchoring configuration.

List of parameters that you can change without any preparatory actions:

* `transaction_fee` - the amount of the fee per byte in satoshis for anchoring
  transactions.
* `anchoring_interval` - the interval in blocks between anchored blocks.

The `anchoring_keys` change procedure is more complicated, you can find the description of this process
in the next section.

## Changing the list of anchoring nodes

* **Excluding node from the anchoring nodes.**

  The simplest case of changing anchoring nodes list is to exclude one of node from anchoring.
  You just have to exclude their keys from the `anchoring_keys` array.

* **Adding a new node to the list of anchoring nodes.**

  In this case you must prepare the candidate node for inclusion in the list of
  anchoring nodes. In according of a [newbie guide][newbie_guide:step-3] you
  should generate Bitcoin keypair for the candidate. After tha configuration
  is applied, you must remember to run the `btc_anchoring_sync` utility.

* **Changing of the bitcoin key of an existing anchoring node.**

  This case is rare and in many ways similar to the previous one, but there
  are some differences. Instead of generating a new config for the sync utility
  you have to add a new Bitcoin keypair to the existing one.

  To do it, run `btc_anchoring_sync` utility:

  ```shell
  cargo run -- generate-keypair -c path/to/anchoring/sync.toml
  ```

  As a result of this call you will obtain a new `bitcoin_key`, which you may
  use to replace the existing one.

[anchoring:actual-address]: https://exonum.com/doc/version/latest/advanced/bitcoin-anchoring/#actual-address
[anchoring:add-funds]: https://exonum.com/doc/version/latest/advanced/bitcoin-anchoring/#add-funds
[exonum_launcher]: https://github.com/popzxc/exonum-launcher
[newbie_guide:step-3]: newbie.md#step-3-deploying-and-running
