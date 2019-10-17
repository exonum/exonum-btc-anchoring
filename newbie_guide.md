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

- Be sure to have [`exonum_launcher`](https://github.com/popzxc/exonum-launcher)
installed via `pip` (see `exonum-launcher` README for details).
- Install `exonum_btc_anchoring_plugin`
    (if you are using `venv`, activate `venv` in which `exonum_launcher` is installed):

    ```sh
    pip install -e exonum-btc-anchoring/launcher
    ```

- Compile and install `btc_anchoring` example:

    ```sh
    cd exonum-btc-anchoring
    cargo install --path . --example btc_anchoring --force
    ```

- Build the `exonum-btc-anchoring` project (it contains a binary that will be used later):

    ```sh
    cargo build
    ```

- Run the example:

    ```sh
    RUST_LOG="exonum_btc_anchoring=info" btc_anchoring run-dev -a anchoring
    ```

    `-a anchoring` here denotes the directory in which the data of the node will be generated.

## Step 3. Deploying and Running

- First of all, obtain `service_key` and `bitcoin_key`.

    To obtain `service_key`, go to the directory where the data of the node lies
    (in our example it is `anchoring`) and open `node.toml`.
    Here you can find `service_key`.

    To obtain `bitcoin_key`, go to the `exonum-btc-anchoring` directory and launch the following command:

    ```sh
    cargo run -- generate-config -o anchoring/sync.toml --bitcoin-rpc-host http://localhost --bitcoin-rpc-user user --bitcoin-rpc-password password
    ```

    In the code above you should replace `anchoring` with the directory where the data of your node lies.

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
        name: "exonum-btc-anchoring:0.12.0"

    instances:
      anchoring:
        artifact: anchoring
        config:
          network: regtest
          anchoring_interval: 500
          transaction_fee: 10
          anchoring_keys:
            - bitcoin_key: "0332ab15173cf9ff8ad0946fbd515434bf1f04bb46482453474b6c38b94fa9d7b7"
              service_key: "2b89c8e1f3a6a8f18ac3276b87403e39c54c33d8275e9626ab9478df4d6d5bfc"
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
      --data '"020000000001051aa3e2a53010c4863becf608fc9bfe1d06c7a2456c820de85ee9e65dc95bcc6300000000171600145058446ed566ce61f9511a28b5f35340bb7bfc9afeffffffbed3ee193e9b73d0b259d60a57cffc2310f228fce4658060e9dffd24514631400000000017160014475caf1c9227353fb8bc10252a634dffbf7d39e1feffffff2787ebd988bfeb449c1482c1877e0598c6d28ef2db426f7c8f83da219d0f23c20000000017160014475caf1c9227353fb8bc10252a634dffbf7d39e1feffffff53a188e10f648e97fa9a4ed84bb0355ad52a301643249217b05f8114e76f579c00000000171600145058446ed566ce61f9511a28b5f35340bb7bfc9afeffffff8676d4e58838387079589229344eea778d0b29d095407f42ecdcf423975537ea0000000017160014475caf1c9227353fb8bc10252a634dffbf7d39e1feffffff0200c817a8040000002200209959c79a416778463632a0a1164e312c5973865dbc9fa038c69fc9e5af71e3cae4c7052a010000001600148e8a01a394875fec3c51c3a8117f9b00b57286d5024730440220287b30723dc3ed88d06f0884611071ba7b547903c2937846befd51cb601701f502206f74e78aa2e74ee8494c22481c1b661f8fcc171f04dfce61ff05a2ab53cbb634012103a54ae66b63b6bdc2089f294d43611ee37deaaf346cffd16f23d5f521dbca757c02473044022012490b04d0623f25661445e9bbcbfc3491b2009763dafdfd15c109e0617278cd022027d9ac08e9877f01b7b6234bd921016b4f7135a32cc25abd2f19a8cddfa692e3012103a208fc3f46c7b815c3237636d21267c2610a975b03dffae657017ab33fa832bd024730440220203514c2473a1ccb3f5722c4afc4347e083669784700cd72d15316d3b8cef3d502207e1120d31288ee91e58499a37f41b88578556474e334d7327b3f47a983be95af012103a208fc3f46c7b815c3237636d21267c2610a975b03dffae657017ab33fa832bd02473044022005bbf3b54f16ccabd7d5d98d26597bc19ce4af14e4952fa571e7d5d83f4a284402202c54adde15a678f7ce57f723e0ea875681e9b5f13aeffcba3871135fc82401de012103a54ae66b63b6bdc2089f294d43611ee37deaaf346cffd16f23d5f521dbca757c02473044022052d4d19bcf133fdcdaf526c0ec0aca362dab5a75ead2fb33b2f7378ef3d8cbcb02206251d0d55680ab0f8463e964755c1a63791e7d6d637a84f907b82c94a18221a4012103a208fc3f46c7b815c3237636d21267c2610a975b03dffae657017ab33fa832bd48000000"' \
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
    RUST_LOG="exonum_btc_anchoring=info" cargo run -- run --config anchoring/sync.toml
    ```

    `anchoring/` in the code above means the directory where `sync.toml` was generated earlier.

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
