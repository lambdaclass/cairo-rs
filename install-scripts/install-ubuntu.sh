#!/usr/bin/env bash
set -ex

# This installation has been tested on Ubuntu 

sudo apt update -y

# Install Rust and Cargo
curl https://sh.rustup.rs -sSf | sh -s -- -y

# Make sure Rust has been installed correctly
rustc --version

# Install pyenv dependencies
sudo apt-get install -y make build-essential libssl-dev zlib1g-dev libbz2-dev libreadline-dev libsqlite3-dev wget llvm libncurses5-dev libncursesw5-dev xz-utils tk-dev libffi-dev liblzma-dev

# Install pyenv
curl https://pyenv.run | bash

# Make sure pyenv has been installed correctly
pyenv -v

# Installing python 3.9 with pyenv
pyenv install 3.9.16

# Setting python 3.9 as the default local version
pyenv local 3.9.16 # is this needed given that we then create a virtual environment?

# Create and enter a virtual environment

python3.9 -m venv ~/cairo_venv
source ~/cairo_venv/bin/activate

# Install cairo dependencies
sudo apt install -y libgmp3-dev
pip3 install ecdsa fastecdsa sympy

# Install cairo
pip3 install cairo-lang

