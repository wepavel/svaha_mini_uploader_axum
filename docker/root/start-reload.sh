#! /usr/bin/env sh
set -e

HOST=${HOST:-0.0.0.0}
PORT=${PORT:-80}
LOG_LEVEL=${LOG_LEVEL:-info}

ls_result=$(ls)
echo "LS $ls_result"

ls_result=$(pwd)
echo "PWD $ls_result"

echo "Host: "$HOST "Port: "$PORT

./app/svaha_mini_uploader_axum 2>&1 | vector --config vector.toml