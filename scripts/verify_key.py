#!/usr/bin/env python3
"""Test Polymarket API auth via curl subprocess."""
import subprocess, time, hmac, hashlib, base64, sys

api_key = sys.argv[2]
api_secret = sys.argv[3]
api_passphrase = sys.argv[4]
PROXY = "http://127.0.0.1:7890"

timestamp = str(int(time.time()))
message = f"{timestamp}GET/balance-allowance"

try:
    secret_bytes = base64.urlsafe_b64decode(api_secret + "==" if len(api_secret) % 4 else api_secret)
except:
    secret_bytes = api_secret.encode()

sig = hmac.new(secret_bytes, message.encode(), hashlib.sha256).digest()
sig_b64 = base64.urlsafe_b64encode(sig).decode().rstrip("=")

url = "https://clob.polymarket.com/balance-allowance?asset_type=COLLATERAL"

# Write curl args to a file to avoid shell quoting issues
import tempfile, os
script = f"""#!/bin/bash
curl -s -w '\\n%{{http_code}}' -x {PROXY} \\
  -H 'POLY_API_KEY: {api_key}' \\
  -H 'POLY_SIGNATURE: {sig_b64}' \\
  -H 'POLY_PASSPHRASE: {api_passphrase}' \\
  -H 'POLY_TIMESTAMP: {timestamp}' \\
  -H 'Content-Type: application/json' \\
  -H 'Accept: */*' \\
  '{url}' --max-time 15
"""

tmpf = tempfile.NamedTemporaryFile(mode='w', suffix='.sh', delete=False)
tmpf.write(script)
tmpf.close()

result = subprocess.run(['bash', tmpf.name], capture_output=True, text=True, timeout=20)
os.unlink(tmpf.name)

output = result.stdout.strip()
if not output:
    print(f"FAIL (no output): {result.stderr[:300]}")
    sys.exit(1)

lines = output.rsplit('\n', 1)
body = lines[0]
code = lines[1] if len(lines) > 1 else "?"

print(f"HTTP {code}")
print(f"Body: {body[:500]}")

if code == "200":
    print("\n✅ Credentials VALID!")
elif code == "401":
    print("\n❌ 401 — credentials INVALID")
else:
    print(f"\n⚠️ Unexpected response")
