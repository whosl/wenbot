#!/bin/bash
# Step 1: Derive existing API key from private key (via curl to bypass Python SSL issues)
PRIVATE_KEY="$1"
PROXY="http://127.0.0.1:7890"
HOST="https://clob.polymarket.com"

echo "=== Step 1: Get wallet address from private key ==="
# Use Python to get address (no network needed)
WALLET_ADDR=$(python3 -c "
from eth_account import Account
acct = Account.from_key('0x$PRIVATE_KEY')
print(acct.address)
" 2>/dev/null)
echo "Wallet: $WALLET_ADDR"

echo ""
echo "=== Step 2: Derive API key ==="
# Use Python py_clob_client but override http client
RESULT=$(python3 -c "
import os
for k in list(os.environ.keys()):
    if 'proxy' in k.lower():
        del os.environ[k]

import subprocess, json

# We need to use curl for network, but py_clob_client for crypto
# Just do the derivation locally - it doesn't need network
from eth_account import Account
from eth_account.messages import encode_defunct
from py_clob_client.client import ClobClient

key = '0x$PRIVATE_KEY'
client = ClobClient('$HOST', key=key, chain_id=137)

# derive_api_key makes a GET request, which we can't do via Python
# Let's just check if we can get the existing key
# Actually, derive constructs a signed request - let's generate it and use curl

import time, hmac, hashlib, base64
timestamp = str(int(time.time()))
path = '/api-key'
message = timestamp + 'GET' + path + ''

# The signature uses the private key (eth sign)
from eth_account.messages import encode_defunct
msg = encode_defunct(text=message)
signed = Account.from_key(key).sign_message(msg)
sig_hex = signed.signature.hex()

# Polymarket expects base64 of the signature
sig_b64 = base64.b64encode(bytes.fromhex(sig_hex)).decode()

print(json.dumps({
    'timestamp': timestamp,
    'signature': sig_b64,
    'address': '$WALLET_ADDR'
}))
" 2>&1)

echo "Derive result: $RESULT"

echo ""
echo "=== Step 3: Call derive endpoint via curl ==="
TS=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['timestamp'])")
SIG=$(echo "$RESULT" | python3 -c "import sys,json; print(json.load(sys.stdin)['signature'])")

echo "Timestamp: $TS"
echo "Signature: ${SIG:0:20}..."

DERIVE_RESP=$(curl -s -x "$PROXY" \
    -H "POLY_ADDRESS: $WALLET_ADDR" \
    -H "POLY_SIGNATURE: $SIG" \
    -H "POLY_TIMESTAMP: $TS" \
    -H "POLY_NONCE: $(python3 -c "import secrets; print(secrets.token_hex(16))")" \
    -H "Content-Type: application/json" \
    "$HOST$path" --max-time 15 2>&1)

echo "Derive response: $DERIVE_RESP"

# Parse the response
echo ""
echo "=== Step 4: Parse results ==="
echo "$DERIVE_RESP" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'api_key' in str(data).lower() or 'apiKey' in str(data):
        print('✅ Got API credentials:')
        print(json.dumps(data, indent=2))
    elif 'error' in str(data).lower():
        print(f'❌ Error: {data}')
    else:
        print(f'Response: {json.dumps(data, indent=2)}')
except:
    print(f'Raw: {sys.stdin.read()[:500]}')
"
