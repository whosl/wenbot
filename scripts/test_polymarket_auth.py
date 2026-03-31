#!/usr/bin/env python3
"""Test Polymarket CLOB API authentication directly."""
import hmac, hashlib, base64, json, sys
from urllib.request import Request, build_opener, ProxyHandler

API_KEY = input("API Key: ").strip()
API_SECRET = input("API Secret: ").strip()
API_PASSPHRASE = input("API Passphrase: ").strip()

proxy = ProxyHandler({'https': 'http://127.0.0.1:7890'})
opener = build_opener(proxy)

# L2 HMAC auth (matching py-clob-client exactly)
ts = str(int(__import__('time').time()))
method = "GET"
path = "/balance-allowance"
body = ""

message = ts + method + path
if body:
    message += body

# Decode secret from base64url, HMAC-SHA256, encode result as base64url
secret_bytes = base64.urlsafe_b64decode(API_SECRET)
h = hmac.new(secret_bytes, message.encode('utf-8'), hashlib.sha256)
sig = base64.urlsafe_b64encode(h.digest()).decode('utf-8')

req = Request(f"https://clob.polymarket.com{path}?asset_type=COLLATERAL", method=method)
req.add_header('User-Agent', 'py-clob-client/0.34.6')
req.add_header('POLY_API_KEY', API_KEY)
req.add_header('POLY_SIGNATURE', sig)
req.add_header('POLY_PASSPHRASE', API_PASSPHRASE)
req.add_header('POLY_TIMESTAMP', ts)

try:
    resp = opener.open(req, timeout=15)
    data = resp.read().decode()
    print(f"✅ SUCCESS (status {resp.status})")
    print(f"Response: {data}")
except Exception as e:
    print(f"❌ FAILED: {e}")
    if hasattr(e, 'read'):
        print(f"Body: {e.read().decode()[:500]}")
