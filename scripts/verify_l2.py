#!/usr/bin/env python3
"""
Verify Polymarket L2 API credentials using curl for network.
"""
import sys, json, subprocess, time, hmac, hashlib, base64

PROXY = "http://127.0.0.1:7890"
HOST = "https://clob.polymarket.com"

with open("/tmp/polymarket_creds.json") as f:
    creds = json.load(f)

api_key = creds["apiKey"]
api_secret = creds["secret"]
api_passphrase = creds["passphrase"]

timestamp = str(int(time.time()))
path = "/balance-allowance"
method = "GET"
body = ""

message = timestamp + method + path + body

# HMAC-SHA256: decode base64url secret, sign, encode result as base64url
secret_bytes = base64.urlsafe_b64decode(api_secret + "==" if len(api_secret) % 4 else api_secret)
sig = hmac.new(secret_bytes, message.encode(), hashlib.sha256).digest()
sig_b64 = base64.urlsafe_b64encode(sig).decode().rstrip("=")

url = f"{HOST}{path}?asset_type=COLLATERAL"

print(f"=== L2 Auth Balance Check ===")
print(f"  API_KEY: {api_key}")
print(f"  Signature: {sig_b64[:20]}...")
print(f"  Timestamp: {timestamp}")

r = subprocess.run(
    ["curl", "-s", "-x", PROXY, "-w", "\\n%{http_code}",
     "-H", f"POLY_API_KEY: {api_key}",
     "-H", f"POLY_SIGNATURE: {sig_b64}",
     "-H", f"POLY_PASSPHRASE: {api_passphrase}",
     "-H", f"POLY_TIMESTAMP: {timestamp}",
     "-H", "Content-Type: application/json",
     "-H", "Accept: */*",
     url, "--max-time", "15"],
    capture_output=True, text=True, timeout=20
)

output = r.stdout.strip()
lines = output.rsplit("\n", 1)
body_resp = lines[0]
code = lines[1] if len(lines) > 1 else "?"

print(f"\n  HTTP {code}")
print(f"  Body: {body_resp[:500]}")

if code == "200":
    print(f"\n✅ L2 Auth SUCCESS! Credentials are valid!")
elif code == "401":
    print(f"\n❌ 401 Unauthorized - credentials rejected by Polymarket")
else:
    print(f"\n⚠️ Unexpected status code")
