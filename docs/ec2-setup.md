# EC2 部署配置文档

> 最后更新: 2026-03-31

## 服务器信息

| 项目 | 值 |
|------|-----|
| 公网 IP | `16.171.27.253` |
| SSH 用户 | `ubuntu` |
| SSH 连接 | `ssh ec2-wenbot` (见下方本地 SSH Config) |
| 区域 | eu-west-1 (Ireland) |
| 系统 | Ubuntu |

## 本地 SSH 配置

文件: `~/.ssh/config`

```
Host ec2-wenbot
    HostName 16.171.27.253
    User ubuntu
    IdentityFile ~/.ssh/Euey.pem
```

## 网络地址

| 服务 | 地址 |
|------|------|
| wenbot 前端 (HTTPS) | `https://wenbot.nas.cpolar.cn` |
| 后端 API (内部) | `http://localhost:8000` |
| cpolar 管理面板 | `http://localhost:9200` |

## 目录结构

```
/home/ubuntu/
├── wenbot/                          # 项目根目录 (git clone)
│   └── rust-backend/
│       ├── virtual_wallet.db        # SQLite 数据库 (~23MB)
│       └── target/release/
│           └── api-server           # Rust 后端二进制
├── pyenv/                           # Python venv
├── restart.sh                       # 重启脚本
├── start_cpolar.sh                  # cpolar 启动脚本
└── wenbot.log                       # 后端日志
```

## 启动脚本

### restart.sh — 重启后端

```bash
#!/bin/bash
pkill -f api-server 2>/dev/null
sleep 1
cd /home/ubuntu/wenbot/rust-backend
touch virtual_wallet.db
nohup ./target/release/api-server > /home/ubuntu/wenbot.log 2>&1 &
sleep 5
echo ===LOG===
tail -30 /home/ubuntu/wenbot.log
echo ===HEALTH===
curl -s http://localhost:8000/api/health
echo
echo ===STATUS===
curl -s http://localhost:8000/api/fivesbot/status
```

### start_cpolar.sh — 启动 cpolar 隧道

```bash
#!/bin/bash
sudo killall cpolar 2>/dev/null
sleep 1
sudo /usr/local/bin/cpolar http 8000 -region eu -daemon on
sleep 6
curl -s http://localhost:9200/api/tunnels | python3 -c '
import json,sys
d = json.load(sys.stdin)
for t in d.get("tunnels",[]):
    print(t["public_url"])
' 2>/dev/null || echo "no tunnel"
```

> ⚠️ `start_cpolar.sh` 里用了 `-region eu`，实际 cpolar.yml 配置的是 `cn_nas`。以 yml 配置为准。

## cpolar 配置

文件: `/usr/local/etc/cpolar/cpolar.yml`

```yaml
authtoken: <redacted>
tunnels:
  wenbot:
    proto: http
    addr: 8000
    region: cn_nas
    subdomain: wenbot
```

- cpolar 以 systemd 服务运行 (`cpolar.service`)
- 自定义子域名: `wenbot`
- 公网 URL: `https://wenbot.nas.cpolar.cn`
- 订阅级别: NAS10M，支持 `cn_nas` region

## Polymarket 配置

### API 认证方式

Rust 后端通过 `POLY_ADDRESS` + `POLY_API_KEY` + `POLY_SIGNATURE` + `POLY_PASSPHRASE` + `POLY_TIMESTAMP` header 做 HMAC-SHA256 L2 认证。

### 凭证存储

凭证通过 SRP + age 加密存储在 SQLite `wallet_config` 表：
- `srp_salt` — SRP 协议盐值
- `srp_verifier` — SRP 协议验证器
- `age_ciphertext` — age 加密的凭证 JSON

加密的凭证 JSON 包含：
```json
{
  "api_key": "...",
  "api_secret": "...",
  "api_passphrase": "...",
  "address": "0x..."
}
```

### 配置流程

```bash
ssh ec2-wenbot
cd ~/wenbot/rust-backend
./target/release/api-server setup-polymarket
# 输入: API key, API secret, API passphrase, wallet address, 解锁密码(两次)
```

### 前端 Connect Wallet

1. 打开 `https://wenbot.nas.cpolar.cn`
2. 输入 setup 时设的解锁密码
3. SRP 握手 → age 解密 → 凭证加载到后端内存
4. 后续 API 请求自动带上 HMAC 签名

### Polymarket API 端点

| 路由 | 说明 |
|------|------|
| `/api/polymarket/balance` | USDC.e 余额 + allowance |
| `/api/polymarket/orders` | 当前挂单 |
| `/api/polymarket/positions` | 当前持仓 (暂返回空) |
| `/api/polymarket/trades` | 交易历史 |

## Python 环境

```
~/pyenv/                    # Python venv
├── py_clob_client==0.34.6  # Polymarket Python SDK
└── eth-account==0.13.7     # 以太坊账户
```

使用: `source ~/pyenv/bin/activate`

### 常用 Python 调试

```bash
source ~/pyenv/bin/activate
python3 -c "
from py_clob_client.client import ClobClient
HOST = 'https://clob.polymarket.com'
CHAIN_ID = 137
client = ClobClient(HOST, key='<PRIVATE_KEY>', chain_id=CHAIN_ID)
creds = client.create_or_derive_api_creds()
client.set_api_creds(creds)
print('Orders:', client.get_orders())
print('API keys:', client.get_api_keys())
"
```

## 数据库同步 (本地 → EC2)

```bash
rsync -avz -e ssh ~/.wenbot/rust-backend/virtual_wallet.db \
  ec2-wenbot:/home/ubuntu/wenbot/rust-backend/virtual_wallet.db
```

## 二进制更新 (本地编译 → EC2)

```bash
# 本地编译
cd ~/wenbot/rust-backend
~/.cargo/bin/cargo build --release -p api-server

# 同步到 EC2
rsync -avz -e ssh ./target/release/api-server \
  ec2-wenbot:/home/ubuntu/wenbot/rust-backend/target/release/api-server

# EC2 上重启
ssh ec2-wenbot 'cd ~/wenbot/rust-backend && bash ~/restart.sh'
```

## 常用排查

```bash
# 查看日志
ssh ec2-wenbot 'tail -50 ~/wenbot.log'

# 查看进程
ssh ec2-wenbot 'ps aux | grep api-server'

# 查看端口
ssh ec2-wenbot 'ss -tlnp | grep 8000'

# 测试 API
ssh ec2-wenbot 'curl -s http://localhost:8000/api/health'
ssh ec2-wenbot 'curl -s http://localhost:8000/api/polymarket/balance'

# 检查 cpolar
ssh ec2-wenbot 'sudo systemctl status cpolar'
ssh ec2-wenbot 'curl -s http://localhost:9200/api/tunnels'
```
