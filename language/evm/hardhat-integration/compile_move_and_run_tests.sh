#!/bin/bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

python3 "$SCRIPT_DIR/compile_move.py" && npx hardhat test --no-compile
