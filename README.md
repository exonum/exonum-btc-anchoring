# Exonum anchoring service

Здесь будет короткое описание проекта, ознакомится с полным описанием анкоринга можно [здесь](link).

# Build steps

Можно посмотреть в крейте `exonum`.

# Bitcoin full node deploy

## Configuration
Для работы сервиса анкоринга необходимо запустить узел `Bitcoin` с конфигурационным файлом `bitcoin.conf` примерно следующего содержания:
```ini
testnet=1 # только для проверке на тестнете
server=1 # для активации rpc
txindex=1 # для того, чтобы нода индексировала все транзакции 

rpcuser=<username>
rpcpassword=<password>
```
Подробную документацию по `bitcoin.conf` можно найти  [здесь](https://en.bitcoin.it/wiki/Running_Bitcoin#Bitcoin.conf_Configuration_File).

## Launching
Запустить узел можно командой
```
$ bitcoind --reindex --daemon
```
И обязательно нужно дождаться полной загрузки всего блокчейна. 
В случае, если узел поднимается для существующего блокчейна, нужно убедиться, что текущий адрес был импортирован при помощи `importaddress`.

# Anchoring testnet example
Для быстрого знакомства с анкорингом можно воспользоваться встроенным примером, который устанавливается командой.
```
$ cargo install --example anchoring
```

## Generate testnet config
Создать конфигурацию тестовой сети можно при помощи команды.
```
$ anchoring generate \
    --output-dir <destdir> <n> \
    --anchoring-host <bitcoin full node host> \
    --anchoring-user <username> \
    --anchoring-password <password> \
    --anchoring-funds <initial funds>
```
Которая создаст конфигурацию на N узлов, используя поднятый bitcoin узел. 

*warning: важно, чтобы баланс, который можно узнать при помощи `getbalance` был больше, чем `<initial_funds>`, так, как фундируюая транзакция создается на этапе генерации тестнета.*

## Launching testnet
Для запуска тестнета нужно запустить все `anchoring` узлы. Команда для запуска `m`-ого узла будет выглядеть так:
```
anchoring run --node-config <destdir>/<m>.toml --leveldb-path <destdir>/db/<m>
```
Дополнительно можно указать порт, через который узел будет принимать предложения по изменению конфигурации. 
Подробное описание команды можно получить при помощи:
```
anchoring --help
```

*warning Не стоит использовать данную утилиту и ее аналоги для реального использования! Существует риск утечки закрытых ключей!*

# Next steps

Техническую документацию можно найти на [сайте](link).