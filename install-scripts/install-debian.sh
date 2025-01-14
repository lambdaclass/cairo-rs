#!/usr/bin/env bash
set -ex

apt update -y

# Install curl in order to install Rust and Cargo
# Install make, necessary for installing python 3.9 with pyenv

apt install -y curl \
               make

# Install Rust and Cargo
curl https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"

# Make sure Rust has been installed correctly
rustc --version

# Install pyenv dependencies
apt-get install -y git make build-essential libssl-dev zlib1g-dev libbz2-dev \
        libreadline-dev libsqlite3-dev wget llvm libncurses5-dev libncursesw5-dev \
        xz-utils tk-dev libffi-dev liblzma-dev libgmp3-dev

# Install pyenv
curl https://pyenv.run | bash
export PYENV_ROOT="$HOME/.pyenv"
command -v pyenv >/dev/null || export PATH="$PYENV_ROOT/bin:$PATH"
eval "$(pyenv init -)"

# Make sure pyenv has been installed correctly
pyenv -v

make uv-program
make deps

echo "-- You need to follow these instructions to finish installing pyenv: --"
pyenv init || true
