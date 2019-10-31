# A Maintenance Guide

This manual is intended for advanced users who already able to launch an anchoring
instance in accordance of a [newbie](newbie.md) guide.

The manual describes most common procedures of the service maintenance:

* [Funding of anchoring chain wallet](#Funding-of-anchoring-chain-wallet)
* [Modification of configuration parameters](#Modification-of-configuration-parameters)
* [Changing list of anchoring nodes](#Changing-list-of-anchoring-nodes)

## Funding of anchoring chain wallet

Sometimes you have to replenish the anchoring chain wallet to keep anchoring going.
To do this send a some amount of Bitcoins to the actual anchoring address, which
you can obtain by the public HTTP API [endpoint][anchoring:actual-address].
And then save received transaction hex and wait until it get enough confirmations
in Bitcoin network. When the transaction receives enough confirmations, send it
to each of the anchoring nodes using the corresponding private HTTP API
[endpoint][anchoring:add-funds].

***Beware!** The anchoring node itself does not check that the funding
transaction is confirmed and can be spend. If you send a malformed transaction,
you will completely break anchoring.*

## Modification of configuration parameters

## Changing list of anchoring nodes

[anchoring:actual-address]: https://exonum.com/doc/version/latest/advanced/bitcoin-anchoring/#actual-address
[anchoring:add-funds]: https://exonum.com/doc/version/latest/advanced/bitcoin-anchoring/#add-funds
