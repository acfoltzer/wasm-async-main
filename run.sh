#!/usr/bin/env bash

set -e

(cd guest && cargo build)
lucetc guest/target/wasm32-wasi/debug/guest.wasm -o target/guest.so --bindings /home/acfoltzer/src/isolation/public/lucet-wasi/bindings.json
cargo run
