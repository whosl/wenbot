import { connectStart, connectVerify, type WalletConnectStartResponse } from './api'

const textEncoder = new TextEncoder()

function hexToBigInt(hex: string): bigint {
  return BigInt(`0x${hex}`)
}

function bigIntToHex(n: bigint): string {
  const hex = n.toString(16)
  return hex.length % 2 === 0 ? hex : `0${hex}`
}

function hexToBytes(hex: string): Uint8Array {
  const normalized = hex.length % 2 === 0 ? hex : `0${hex}`
  const out = new Uint8Array(normalized.length / 2)
  for (let i = 0; i < out.length; i++) out[i] = parseInt(normalized.slice(i * 2, i * 2 + 2), 16)
  return out
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes).map((b) => b.toString(16).padStart(2, '0')).join('')
}

function concatBytes(...parts: Uint8Array[]): Uint8Array {
  const size = parts.reduce((n, p) => n + p.length, 0)
  const out = new Uint8Array(size)
  let offset = 0
  for (const part of parts) {
    out.set(part, offset)
    offset += part.length
  }
  return out
}

async function sha256(data: Uint8Array): Promise<Uint8Array> {
  const hash = await crypto.subtle.digest('SHA-256', data)
  return new Uint8Array(hash)
}

function modPow(base: bigint, exponent: bigint, modulus: bigint): bigint {
  if (modulus === 1n) return 0n
  let result = 1n
  let b = base % modulus
  let e = exponent
  while (e > 0n) {
    if (e & 1n) result = (result * b) % modulus
    e >>= 1n
    b = (b * b) % modulus
  }
  return result
}

function padToN(value: bigint, nBytes: number): Uint8Array {
  const raw = hexToBytes(bigIntToHex(value))
  if (raw.length >= nBytes) return raw
  const out = new Uint8Array(nBytes)
  out.set(raw, nBytes - raw.length)
  return out
}

function b64(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
}

async function randomBigInt(bytes = 32): Promise<bigint> {
  const buf = new Uint8Array(bytes)
  crypto.getRandomValues(buf)
  return hexToBigInt(bytesToHex(buf))
}

async function computeX(username: string, password: string, saltHex: string): Promise<bigint> {
  const inner = await sha256(textEncoder.encode(`${username}:${password}`))
  const salt = hexToBytes(saltHex)
  return hexToBigInt(bytesToHex(await sha256(concatBytes(salt, inner))))
}

async function computeFileKey(password: string, saltHex: string): Promise<string> {
  return bytesToHex(await sha256(textEncoder.encode(`wenbot:polymarket:file-key:v1:${saltHex}:${password}`)))
}

async function aesGcmEncrypt(keyBytes: Uint8Array, plaintext: string): Promise<{ iv: string; ciphertext: string }> {
  const key = await crypto.subtle.importKey('raw', keyBytes, { name: 'AES-GCM' }, false, ['encrypt'])
  const iv = crypto.getRandomValues(new Uint8Array(12))
  const ciphertext = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, textEncoder.encode(plaintext)))
  return { iv: b64(iv), ciphertext: b64(ciphertext) }
}

export async function connectWalletWithPassword(password: string): Promise<void> {
  const start = await connectStart('polymarket')
  await completeSrpHandshake(start, password)
}

async function completeSrpHandshake(start: WalletConnectStartResponse, password: string): Promise<void> {
  const username = 'polymarket'
  const N = hexToBigInt(start.n_hex)
  const g = hexToBigInt(start.g_hex)
  const B = hexToBigInt(start.B)
  const nBytes = hexToBytes(start.n_hex).length

  const a = await randomBigInt(32)
  const A = modPow(g, a, N)
  const x = await computeX(username, password, start.salt)
  const k = hexToBigInt(bytesToHex(await sha256(concatBytes(padToN(N, nBytes), padToN(g, nBytes)))))
  const u = hexToBigInt(bytesToHex(await sha256(concatBytes(padToN(A, nBytes), padToN(B, nBytes)))))
  const gx = modPow(g, x, N)
  let base = (B - (k * gx) % N + N) % N
  if (base === 0n) throw new Error('Invalid SRP server value')
  const exp = a + u * x
  const S = modPow(base, exp, N)
  const K = await sha256(padToN(S, nBytes))
  const m1 = bytesToHex(await sha256(concatBytes(padToN(A, nBytes), padToN(B, nBytes), K)))
  const fileKeyHex = await computeFileKey(password, start.salt)
  const wrapped = await aesGcmEncrypt(K, fileKeyHex)

  const res = await connectVerify({
    session_id: start.session_id,
    A: bigIntToHex(A),
    M1: m1,
    wrapped_file_key: wrapped.ciphertext,
    wrapped_file_key_iv: wrapped.iv,
  })

  const expectedM2 = bytesToHex(await sha256(concatBytes(padToN(A, nBytes), hexToBytes(m1), K)))
  if (res.M2.toLowerCase() !== expectedM2.toLowerCase()) {
    throw new Error('Server proof verification failed')
  }
}
