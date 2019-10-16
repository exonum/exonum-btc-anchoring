from exonum_launcher.instances import InstanceSpecLoader

from exonum_client.protobuf_loader import ProtobufLoader
from exonum_client.module_manager import ModuleManager
from exonum_client.proofs.encoder import build_encoder_function

from exonum_launcher.configuration import Instance

from exonum_launcher.instances.instance_spec_loader import InstanceSpecLoader, InstanceSpecLoadError


def import_or_load_module(loader: ProtobufLoader, instance: Instance, module_name: str):
    try:
        # Try to load module (if it's already compiled) first.
        module = ModuleManager.import_service_module(
            instance.artifact.name, module_name)
        return module
    except (ModuleNotFoundError, ImportError):
        # If it's not compiled, load & compile protobuf.
        loader.load_service_proto_files(
            instance.artifact.runtime_id, instance.artifact.name)
        module = ModuleManager.import_service_module(
            instance.artifact.name, "service")
        return module


def bitcoin_network_from_string(network_string: str) -> int:
    match = {
        "bitcoin": 0xD9B4BEF9,
        "testnet": 0x0709110B,
        "regtest": 0xDAB5BFFA
    }
    return match[network_string]


class AnchoringInstanceSpecLoader(InstanceSpecLoader):
    """Spec loader for btc anchoring."""

    def load_spec(self, loader: ProtobufLoader, instance: Instance) -> bytes:
        try:
            service_module = import_or_load_module(loader, instance, "service")
            btc_types_module = import_or_load_module(
                loader, instance, "btc_types")
            helpers_module = import_or_load_module(loader, instance, "helpers")

            # Create config message
            config = service_module.Config()
            config.network = bitcoin_network_from_string(
                instance.config["network"])
            config.anchoring_interval = instance.config["anchoring_interval"]
            config.transaction_fee = instance.config["transaction_fee"]

            anchoring_keys = []
            for keypair in instance.config["anchoring_keys"]:
                service_key = helpers_module.PublicKey(
                    data=bytes.fromhex(keypair["service_key"]))
                bitcoin_key = btc_types_module.PublicKey(
                    data=bytes.fromhex(keypair["bitcoin_key"]))

                anchoring_keys_type = service_module.AnchoringKeys()
                anchoring_keys_type.service_key.CopyFrom(service_key)
                anchoring_keys_type.bitcoin_key.CopyFrom(bitcoin_key)
                anchoring_keys.append(anchoring_keys_type)
            config.anchoring_keys.extend(anchoring_keys)

            result = config.SerializeToString()

        # We're catching all the exceptions to shutdown gracefully (on the caller side) just in case.
        # pylint: disable=broad-except
        except Exception as error:
            artifact_name = instance.artifact.name
            raise InstanceSpecLoadError(
                f"Couldn't get a proto description for artifact: {artifact_name}, error: {error}"
            )

        return result
