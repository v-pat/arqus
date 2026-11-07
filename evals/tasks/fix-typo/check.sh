#!/bin/sh
set -eu

expected='# Example project

We receive input from the user.
The parser will receive a line and receive a token.
Keep this line unchanged.'

if grep -q 'recieve' README.md; then
    echo "README.md still contains recieve"
    exit 1
fi

actual=$(cat README.md)
if [ "$actual" != "$expected" ]; then
    echo "README.md changed more than the typo"
    exit 1
fi
