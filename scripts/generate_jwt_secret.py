#!/usr/bin/env python3
"""
Generate a cryptographically secure random JWT_SECRET for HS256.

Usage:
  python3 scripts/generate_jwt_secret.py
  python3 scripts/generate_jwt_secret.py --bytes 64
  python3 scripts/generate_jwt_secret.py --export  # export-friendly
"""
import argparse
import secrets

def generate_jwt_secret(num_bytes: int = 64) -> str:
    # token_urlsafe uses base64url without padding; 64 bytes -> 86 chars
    return secrets.token_urlsafe(num_bytes)

def main():
    p = argparse.ArgumentParser(description="Generate random JWT_SECRET")
    p.add_argument("--bytes", type=int, default=64, help="random bytes (default: 64, min: 32)")
    p.add_argument("--export", action="store_true", help="output as export statement")
    args = p.parse_args()

    if args.bytes < 32:
        p.error("--bytes must be >= 32 for HS256 security")

    secret = generate_jwt_secret(args.bytes)
    if args.export:
        print(f"export JWT_SECRET='{secret}'")
    else:
        print(f"JWT_SECRET={secret}")
        print()
        print("# Add to .env:")
        print(f"echo \"JWT_SECRET={secret}\" >> .env")

if __name__ == "__main__":
    main()
