# A Complete Beginner Guide to BTC Anchoring

This manual is intended for people who are about to use `exonum-btc-anchoring` for the first time.

The manual describes the process of compiling and launching the `btc_anchoring` example within the
`run-dev` mode on Linux.

## Step 1. Preparing `bitcoind`

- Download `bitcoind`: [https://bitcoin.org/en/download](https://bitcoin.org/en/download).
- Install `bitcoind`:

    ```sh
    tar xzf bitcoin-0.18.0-x86_64-linux-gnu.tar.gz
    sudo install -m 0755 -o root -g root -t /usr/local/bin bitcoin-0.18.0/bin/*
    ```

- Create a Bitcoin config file: `~/.bitcoin/bitcoin.conf` with the following content:

    ```sh
    server=1
    regtest=1
    txindex=1
    rpcuser=user
    rpcpassword=password
    ```

- Run the `bitcoind` in the daemon mode:

    ```sh
    bitcoind --daemon
    ```

- Now `bitcoind` is running in the `regtest` mode. Mine some blocks to obtain balance on
your account.

    Get the address of your wallet:

    ```sh
    bitcoin-cli -regtest getnewaddress
    ```

    Mine 100 blocks (change the address to the address you have obtained with the previous command):

    ```sh
    bitcoin-cli -regtest generatetoaddress 100 2NFNp5RbTyEwV8yijYg9sUCHsVApiqov8DA
    ```

    Verify that the balance is now non-zero:

    ```sh
    bitcoin-cli -regtest getbalance
    ```

    If the balance is still zero, generate 100 more blocks.

## Step 2. Compilation and Initial Run

- Be sure to have [`exonum_launcher`](https://github.com/exonum/exonum-launcher)
installed via `pip3` (see `exonum-launcher` README for details).
- Install `exonum_btc_anchoring_plugin`
    (if you are using `venv`, activate `venv` in which `exonum_launcher` is installed):

    ```sh
    pip3 install -e exonum-btc-anchoring/launcher
    ```

- Run the example:

    ```sh
    RUST_LOG="exonum_btc_anchoring=info" cargo run --example btc_anchoring run-dev -a target/anchoring
    ```

    `-a target/anchoring` here denotes the directory in which the data of the node will be generated.

## Step 3. Deploying and Running

- First of all, obtain `service_key` and `bitcoin_key`.

    To obtain `service_key`, go to the directory where the data of the node lies
    (in our example it is `anchoring`) and open `node.toml`.
    Here you can find `service_key`.

    To obtain `bitcoin_key`, go to the `exonum-btc-anchoring` directory and launch the following command:

    ```sh
    cargo run -- generate-config -o target/anchoring/sync.toml --bitcoin-rpc-host http://localhost:18332 --bitcoin-rpc-user user --bitcoin-rpc-password password
    ```

    In the code above you should replace `target/anchoring` with the directory where the data of
    your node lies.

    As a result of this call you will obtain `bitcoin_key`.
- Create file `anchoring.yml` with the following contents:

    ```yaml
    plugins:
      runtime: {}
      artifact:
        anchoring: "exonum_btc_anchoring_plugin.AnchoringInstanceSpecLoader"

    networks:
      - host: "127.0.0.1"
        ssl: false
        public-api-port: 8080
        private-api-port: 8081

    deadline_height: 10000

    artifacts:
      anchoring:
        runtime: rust
        name: "exonum-btc-anchoring"
        version: "0.13.0-rc.2"

    instances:
      anchoring:
        artifact: anchoring
        config:
          network: testnet
          anchoring_interval: 500
          transaction_fee: 10
          anchoring_keys:
            - bitcoin_key: "02d6086aaccc86e6a711ac84ff21a266684c17d188aa7c4eeab0c0f12133308584"
              service_key: "850eb20eebe0b07cf2721ecc9c90aa465a96413dccafad11045a9cb8abf04ed0"
    ```

    Replace `bitcoin_key` and `service_key` with values obtained in the previous step.
- Run `exonum_launcher` to start & deploy the instance:

    ```sh
    python3 -m exonum_launcher -i anchoring.yml
    ```

    If everything was done correctly, service should start successfully.

    Enabling anchoring is a separate step. We will describe it below.

## Step 4. Enabling Anchoring

- To get anchoring working, send some bitcoins to the anchoring node and
setup a funding transaction.

    First of all, obtain the address of the anchoring wallet:

    ```sh
    curl 'http://127.0.0.1:8080/api/services/anchoring/address/actual'
    ```

- Then send some bitcoins to that address and obtain the raw transaction
    (replace the address with the obtained one):

    ```sh
    bitcoin-cli -regtest sendtoaddress bcrt1qn9vu0xjpvauyvd3j5zs3vn3393vh8pjahj06qwxxnly7ttm3u09qhpexa8 200.00
    ```

    After invoking this method you will obtain the transaction hash. Convert it into a raw transaction
    (replace the hash with the obtained one):

    ```sh
    bitcoin-cli -regtest getrawtransaction 2c2faad68e056608c1f8a3cc8b5da0ca8f8846c42bc5e7152bff786882342b76
    ```

- Send this transaction to the anchoring node (replace the data with the obtained raw transaction):

    ```sh
    curl --header "Content-Type: application/json" \
      --request POST \
      --data '"0200000000010151a7dcd1c2829f9c0a93ae6b054e9777528e88e3e0403c4313cf8cf41b27d1730000000000feffffff0240420f0000000000220020f86c30b7ec3496572220f40b21096b74dc5182942b8811d1bb0b3ab21e52b1337007360000000000160014e16cbf1202193f7de0eb058e0dc2b57cbc63d4040247304402203e23349dcda80acc85e94ada52269baf09624afeb794b696fb53f0f37d130f850220599eaa9bb50d5e14269228f4f5d63826d5554275877b5ffd77eca3cd3b1c408e012102604e1c50f8bdaec165e0bc7b81e608709f510c5bf4b18b6aefaf3996317fd9cf77641900"' \
      http://127.0.0.1:8081/api/services/anchoring/add-funds
    ```

    After that step the following information will appear in the log of the example:

    ```sh
    [2019-10-17T09:52:51.482127809Z INFO  exonum_btc_anchoring::blockchain::transactions] ====== ADD_FUNDS ======
    [2019-10-17T09:52:51.482197097Z INFO  exonum_btc_anchoring::blockchain::transactions] txid: 4b252989ed7596bf08107b3a07a5225b3f42db9bd71868d64ca09bab7ebcce89
    [2019-10-17T09:52:51.482206185Z INFO  exonum_btc_anchoring::blockchain::transactions] balance: 20000000000
    ```

- Finally, run the sync tool:

    ```sh
    cd exonum-btc-anchoring
    RUST_LOG="exonum_btc_anchoring=info" cargo run -- run --config target/anchoring/sync.toml
    ```

    `target/anchoring/` in the code above means the directory where `sync.toml` was generated earlier.

    On the `regtest` it will exit with an error, since blocks should be mined manually.
    The log of the example will show that anchoring was made:

    ```sh
    [2019-10-17T09:54:22.057856655Z INFO  exonum_btc_anchoring::blockchain::transactions] ====== ANCHORING ======
    [2019-10-17T09:54:22.057892594Z INFO  exonum_btc_anchoring::blockchain::transactions] txid: 033f2d08720d7774e6a92cb6c6a9539d8bcf2a3ed0121555148cbd9cecb8cf0f
    [2019-10-17T09:54:22.057897939Z INFO  exonum_btc_anchoring::blockchain::transactions] height: 0
    [2019-10-17T09:54:22.057903560Z INFO  exonum_btc_anchoring::blockchain::transactions] hash: 10617dd0945cc9d0239b3f3cb36ac6fb0df7c23ff2dc0a6b0d0e8d372655c790
    [2019-10-17T09:54:22.057908057Z INFO  exonum_btc_anchoring::blockchain::transactions] balance: 19999998470
    ```

    Hooray!
