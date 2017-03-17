# Exonum anchoring service

Здесь будет короткое описание проекта

# Build steps

Аналогичие как для `exonum`

# Bitcoin full node 

Для работы сервиса анкоринга необходимо запустить узел `Bitcoin` в следующей конфигурации
```ini
testnet=1 # только для проверке на тестнете
server=1 # для активации rpc
txindex=1 # для того, чтобы нода индексировала все транзакции 

rpcuser=<username>
rpcpassword=<password>
```

Запускаем узел
```
$ bitcoind --reindex --daemon
```
И обязательно дожидаемся, пока он скачает весь блокчейн

## Разбор кода примера anchoring 

Код для генерации начального конфига для сервиса анкоринга
```rust
let (anchoring_genesis, anchoring_nodes) =
        generate_anchoring_config(&AnchoringRpc::new(rpc.clone()),
                                    btc::Network::Testnet,
                                    count,
                                    total_funds);

    let node_cfgs = generate_testnet_config(count, start_port);
    let dir = dir.join("validators");
    for (idx, node_cfg) in node_cfgs.into_iter().enumerate() {
        let cfg = ServicesConfig {
            node: node_cfg,
            anchoring_service: AnchoringServiceConfig {
                genesis: anchoring_genesis.clone(),
                node: anchoring_nodes[idx].clone(),
            },
        };
        let file_name = format!("{}.toml", idx);
        ConfigFile::save(&cfg, &dir.join(file_name)).unwrap();
    }
```

## Запуск бинарника anchoring

# Генерация конфигурации для тестнета

