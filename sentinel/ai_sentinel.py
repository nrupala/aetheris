import requests
import json
import sys
import os

AETHERIS_AI = os.environ.get("AI_ENDPOINT", "http://aetheris_ai:11434")
VAULT_PATH = os.environ.get("VAULT_PATH", "/data/vault")
AUDIT_LOG = os.path.join(VAULT_PATH, "audit.log")

def analyze_health():
    print("AI Sentinel: Starting health analysis...")
    
    recent_logs = []
    if os.path.exists(AUDIT_LOG):
        with open(AUDIT_LOG, "r") as f:
            recent_logs = f.readlines()[-50:]
    
    prompt = f"""Analyze these system logs for patterns of a brute-force attack or hardware failure.
If you see multiple failed authentication attempts or unusual patterns, respond CRITICAL.
If you see warning signs like increasing latency or memory usage, respond WARNING.
If the system appears healthy, respond SAFE.
Only respond with one word: SAFE, WARNING, or CRITICAL.

Logs:
{''.join(recent_logs)}"""
    
    try:
        response = requests.post(
            f"{AETHERIS_AI}/api/generate",
            json={
                "model": "mistral",
                "prompt": prompt,
                "stream": False
            },
            timeout=30
        )
        
        if response.status_code == 200:
            result = response.json().get("response", "UNKNOWN").strip().upper()
            if result in ["SAFE", "WARNING", "CRITICAL"]:
                print(f"AI Sentinel Prediction: {result}")
                return result
        
        print("AI Sentinel Prediction: SAFE (default)")
        return "SAFE"
        
    except Exception as e:
        print(f"AI Sentinel Error: {e}")
        print("AI Sentinel Prediction: SAFE (error fallback)")
        return "SAFE"

if __name__ == "__main__":
    status = analyze_health()
    sys.exit(0 if status == "SAFE" else 1)
