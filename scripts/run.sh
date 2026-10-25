#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
suwa_runtime="$PWD/.local/gpu/onnxruntime-linux-x64-gpu-1.24.2/lib/libonnxruntime.so"
if [[ -z "${ORT_DYLIB_PATH:-}" && -f "$suwa_runtime" ]]; then
    export ORT_DYLIB_PATH="$suwa_runtime"
    export LD_LIBRARY_PATH="$PWD/.local/gpu/cudnn/nvidia/cudnn/lib:/opt/cuda/lib64${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
if [[ -f "$PWD/.local/models/fast-2x.json" ]]; then
    export SUWA_DEFAULT_MODEL="$PWD/.local/models/fast-2x.json"
elif [[ -f "$PWD/.local/models/cunet-2x.json" ]]; then
    export SUWA_DEFAULT_MODEL="$PWD/.local/models/cunet-2x.json"
fi
export SUWA_MODEL_DIR="$PWD/.local/models"
cargo build --locked --release --features cuda
exec target/release/suwa "$@"
