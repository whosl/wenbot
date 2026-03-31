#!/usr/bin/env python3
"""
Derive Polymarket API key using Python for crypto + curl for network.
Avoids Python httpx SSL issues with Clash proxy.
"""
import sys, json, subprocess, time, os

PRIVATE_KEY = "0x" + sys.argv[1]
PROXY = "http://127.0.0.1:7890"
HOST = "https://clob.polymarket.com"
CHAIN_ID = 137

# ─── Step 1: Crypto (pure Python, no network) ───
from eth_account import Account
from py_clob_client.client import ClobClient
from py_clob_client.headers.headers import create_level_1_headers

client = ClobClient(HOST, key=PRIVATE_KEY, chain_id=CHAIN_ID)
headers = create_level_1_headers(client.signer)

print("=== L1 Headers (for derive) ===")
print(f"  POLY_ADDRESS: {headers['POLY_ADDRESS']}")
print(f"  POLY_TIMESTAMP: {headers['POLY_TIMESTAMP']}")
print(f"  POLY_NONCE: {headers['POLY_NONCE']}")
print(f"  POLY_SIGNATURE: {headers['POLY_SIGNATURE'][:30]}...")

# ─── Step 2: Network via curl ───
endpoint = f"{HOST}/auth/derive-api-key"
print(f"\n=== Deriving API key from {endpoint} ===")

r = subprocess.run(
    ["curl", "-s", "-x", PROXY,
     "-H", f"POLY_ADDRESS: {headers['POLY_ADDRESS']}",
     "-H", f"POLY_SIGNATURE: {headers['POLY_SIGNATURE']}",
     "-H", f"POLY_TIMESTAMP: {headers['POLY_TIMESTAMP']}",
     "-H", f"POLY_NONCE: {headers['POLY_NONCE']}",
     "-H", "Content-Type: application/json",
     "-H", "Accept: */*",
     endpoint, "--max-time", "15"],
    capture_output=True, text=True, timeout=20
)

print(f"  HTTP response: {r.stdout[:500]}")

try:
    data = json.loads(r.stdout)
    if "apiKey" in data:
        print(f"\n✅ API Key derived successfully!")
        print(f"  API_KEY: {data['apiKey']}")
        print(f"  API_SECRET: {data['secret'][:8]}...")
        print(f"  PASSPHRASE: {data['passphrase'][:8]}...")

        # ─── Step 3: Verify with L2 auth ───
        print(f"\n=== Verifying credentials (L2 balance check) ===")
        creds = {
            "apiKey": data["apiKey"],
            "secret": data["secret"],
            "passphrase": data["passphrase"],
        }

        # Save for later use
        with open("/tmp/polymarket_creds.json", "w") as f:
            json.dump({"private_key": sys.argv[1], **creds}, f, indent=2)
        print("  Credentials saved to /tmp/polymarket_creds.json")
    elif "error" in str(data).lower():
        print(f"\n❌ Error: {data}")
    else:
        print(f"\n⚠️ Unexpected response")
except json.JSONDecodeError:
    print(f"\n⚠️ Could not parse response (possibly network error)")
    if r.stderr:
        print(f"  stderr: {r.stderr[:300]}")
