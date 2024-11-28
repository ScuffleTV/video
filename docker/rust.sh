set -ex

apt-get update
apt-get install -y --no-install-recommends \
        build-essential \
        curl \
        ca-certificates

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
