#!/usr/bin/env python
from distutils.core import setup

install_requires = ["exonum-launcher==0.2.0"]

python_requires = ">=3.6"

setup(
    name="exonum_btc_anchoring_plugin",
    version="0.1",
    description="Exonum BTC anchoring plugin",
    url="https://github.com/exonum/exonum-btc-anchoring",
    packages=["exonum_btc_anchoring_plugin"],
    install_requires=install_requires,
    python_requires=python_requires,
)
