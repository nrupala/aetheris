import requests
import json
import sys
import os

AETHERIS_AI = os.environ.get("AI_ENDPOINT", "http://ollama:11434")
DEFAULT_MODEL = os.environ.get("AI_MODEL", "qwen2.5:14b")
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
            f"{AETHERIS_AI}/v1/chat/completions",
            json={
                "model": DEFAULT_MODEL,
                "messages": [{"role": "user", "content": prompt}],
                "temperature": 0.1,
                "stream": False
            },
            timeout=120
        )
        
        if response.status_code == 200:
            msg = response.json()["choices"][0]["message"]
            content = msg.get("content", "") or msg.get("reasoning_content", "")
            result = content.strip().upper()
            for status in ["CRITICAL", "WARNING", "SAFE"]:
                if status in result:
                    print(f"AI Sentinel Prediction: {status}")
                    return status
        
        print("AI Sentinel Prediction: SAFE (default)")
        return "SAFE"
        
    except Exception as e:
        print(f"AI Sentinel Error: {e}")
        print("AI Sentinel Prediction: SAFE (error fallback)")
        return "SAFE"

if __name__ == "__main__":
    import time
    while True:
        try:
            status = analyze_health()
            print(f"Sentinel check complete. Next check in 60 seconds.")
            time.sleep(60)
        except KeyboardInterrupt:
            print("Sentinel shutting down.")
            break
        except Exception as e:
            print(f"Sentinel error: {e}. Retrying in 30 seconds.")
            time.sleep(30)
