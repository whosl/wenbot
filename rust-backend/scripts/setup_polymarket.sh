#!/bin/bash
set -euo pipefail

cd /home/wenzhuolin/wenbot/rust-backend
~/.cargo/bin/cargo run --release -p api-server -- setup-polymarket
