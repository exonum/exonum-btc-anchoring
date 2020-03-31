from exonum_launcher.instances import InstanceSpecLoader

from exonum_client.protobuf_loader import ProtobufLoader
from exonum_client.module_manager import ModuleManager
from exonum_client.proofs.encoder import build_encoder_function

from exonum_launcher.configuration import Instance

from exonum_launcher.instances.instance_spec_loader import InstanceSpecLoader, InstanceSpecLoadError

RUST_RUNTIME_ID = 0
ANCHORING_ARTIFACT_NAME = "exonum-btc-anchoring"
ANCHORING_ARTIFACT_VERSION = "1.0.0"


def import_anchoring_module(name: str):
    return ModuleManager.import_service_module(
        ANCHORING_ARTIFACT_NAME, ANCHORING_ARTIFACT_VERSION, name)


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
            # Load proto files for the Exonum anchoring service:
            loader.load_service_proto_files(
                RUST_RUNTIME_ID, ANCHORING_ARTIFACT_NAME, ANCHORING_ARTIFACT_VERSION)

            service_module = import_anchoring_module("service")
            btc_types_module = import_anchoring_module("btc_types")
            exonum_types_module = import_anchoring_module("exonum.crypto.types")

            # Create config message
            config = service_module.Config()
            config.network = bitcoin_network_from_string(
                instance.config["network"])
            config.anchoring_interval = instance.config["anchoring_interval"]
            config.transaction_fee = instance.config["transaction_fee"]

            anchoring_keys = []
            for keypair in instance.config["anchoring_keys"]:
                service_key = exonum_types_module.PublicKey(
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
