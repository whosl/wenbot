import os
# Polymarket CLOB 不需要代理，直连即可。Clash TUN 会导致 SSL 握手失败。
os.environ['NO_PROXY'] = '*'
os.environ['no_proxy'] = '*'
# 同时清除代理变量，确保 py_clob_client 底层的 httpx 不会走代理
os.environ.pop('HTTP_PROXY', None)
os.environ.pop('HTTPS_PROXY', None)
os.environ.pop('http_proxy', None)
os.environ.pop('https_proxy', None)

from py_clob_client.client import ClobClient

key = input('Private key (0x...): ').strip()
client = ClobClient('https://clob.polymarket.com', key=key, chain_id=137)

# 先试恢复已有的 key
try:
    creds = client.derive_api_key()
    print('API_KEY:', creds.api_key)
    print('API_SECRET:', creds.api_secret)
    print('PASSPHRASE:', creds.api_passphrase)
except Exception as e:
    print(f'Derive failed: {e}')
    # 如果恢复也失败，尝试删除旧的再创建新的
    try:
        client.delete_api_key()
        print('Old key deleted')
    except:
        pass
    creds = client.create_api_key()
    print('API_KEY:', creds.api_key)
    print('API_SECRET:', creds.api_secret)
    print('PASSPHRASE:', creds.api_passphrase)
