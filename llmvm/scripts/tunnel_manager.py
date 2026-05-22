"""
Cloudflare Tunnel Connection Manager — manages the VM's outbound tunnel to Cloudflare.

This runs ON the Oracle VM after cloud-init bootstraps everything.
It creates the tunnel, configures the DNS record, and starts the daemon.

No inbound ports. No public IP exposure. Just outbound HTTPS to Cloudflare edge.
"""

import subprocess
import sys
import json
import os
from dataclasses import dataclass


@dataclass
class TunnelConfig:
    """Tunnel configuration parameters."""
    name: str
    domain: str
    subdomain: str
    target_port: int = 1234
    credentials_dir: str = "/etc/cloudflared"

    @property
    def hostname(self) -> str:
        return f"{self.subdomain}.{self.domain}"

    @property
    def cred_file(self) -> str:
        return os.path.join(self.credentials_dir, f"{self.name}.json")


class TunnelManager:
    """Manage Cloudflare Tunnel lifecycle."""

    def __init__(self, config: TunnelConfig):
        self.config = config

    def _run(self, *args, check=True) -> subprocess.CompletedProcess:
        """Run cloudflared command."""
        cmd = ["cloudflared"] + list(args)
        print(f"  → {' '.join(cmd)}")
        return subprocess.run(cmd, capture_output=True, text=True, check=check)

    def create_tunnel(self) -> str:
        """
        Create a new Cloudflare Tunnel.
        Returns tunnel ID.
        """
        os.makedirs(self.config.credentials_dir, exist_ok=True)

        result = self._run(
            "tunnel", "create",
            "--credentials-file", self.config.cred_file,
            self.config.name
        )

        # Extract tunnel ID from output
        for line in result.stdout.split("\n"):
            if "Created" in line and "unnel" in line.lower():
                tunnel_id = line.split()[-1]
                print(f"  ✅ Tunnel created: {tunnel_id}")
                return tunnel_id

        print(f"  ⚠️  Could not parse tunnel ID from output")
        return ""

    def configure_tunnel(self):
        """Write tunnel configuration file."""
        config_yaml = f"""
tunnel: {self.config.name}
credentials-file: {self.config.cred_file}

protocol: quic

ingress:
  - hostname: {self.config.hostname}
    service: http://localhost:{self.config.target_port}
  - service: http_status:404
"""
        config_path = f"/etc/cloudflared/config.yml"
        with open(config_path, 'w') as f:
            f.write(config_yaml.strip() + "\n")
        print(f"  ✅ Config written to {config_path}")

    def create_dns_record(self):
        """
        Create DNS CNAME record pointing to tunnel.
        Uses cloudflared tunnel route dns command.
        """
        self._run(
            "tunnel", "route", "dns",
            self.config.name,
            self.config.hostname
        )
        print(f"  ✅ DNS record created: {self.config.hostname} → tunnel")

    def install_service(self):
        """Install cloudflared as a systemd service."""
        self._run("service", "install",
                  "--config", "/etc/cloudflared/config.yml")
        print(f"  ✅ Systemd service installed")

    def start(self):
        """Start the tunnel daemon."""
        subprocess.run(["systemctl", "start", "cloudflared"], check=True)
        subprocess.run(["systemctl", "enable", "cloudflared"], check=True)
        print(f"  ✅ Tunnel started and enabled")

    def status(self) -> bool:
        """Check if tunnel is running."""
        try:
            result = subprocess.run(
                ["systemctl", "is-active", "cloudflared"],
                capture_output=True, text=True
            )
            return result.stdout.strip() == "active"
        except Exception:
            return False

    def setup(self) -> bool:
        """Full setup: create → configure → DNS → service → start."""
        print("\n🔧 Setting up Cloudflare Tunnel...")

        try:
            # Check if tunnel already exists
            existing = self._run("tunnel", "list", check=False)
            if self.config.name in existing.stdout:
                print(f"  ℹ️  Tunnel '{self.config.name}' already exists, skipping create")
            else:
                self.create_tunnel()

            self.configure_tunnel()
            self.create_dns_record()
            self.install_service()
            self.start()

            print(f"\n✅ Tunnel ready: https://{self.config.hostname}")
            return True

        except Exception as e:
            print(f"\n❌ Tunnel setup failed: {e}")
            return False

    def verify(self) -> bool:
        """Verify tunnel is working by testing the endpoint."""
        import requests

        url = f"https://{self.config.hostname}/v1/models"
        print(f"\n🔍 Testing: {url}")

        try:
            resp = requests.get(url, timeout=10)
            if resp.status_code == 200:
                data = resp.json()
                models = data.get("data", [])
                print(f"  ✅ Connected! {len(models)} models available")
                for m in models:
                    print(f"     - {m['id']}")
                return True
            else:
                print(f"  ⚠️  Status: {resp.status_code}")
                print(f"  Response: {resp.text[:200]}")
                return False
        except requests.ConnectionError:
            print(f"  ❌ Connection failed — tunnel may not be ready yet")
            return False


def main():
    """CLI entry point."""
    import argparse

    parser = argparse.ArgumentParser(description="Cloudflare Tunnel Manager")
    subparsers = parser.add_subparsers(dest="command")

    subparsers.add_parser("setup", help="Full tunnel setup")
    subparsers.add_parser("status", help="Check tunnel status")
    subparsers.add_parser("verify", help="Test endpoint connectivity")

    args = parser.parse_args()

    config = TunnelConfig(
        name=os.getenv("TUNNEL_NAME", "llmvm-tunnel"),
        domain=os.getenv("TUNNEL_DOMAIN", "nrupalakolkar.com"),
        subdomain=os.getenv("TUNNEL_SUBDOMAIN", "ai")
    )

    manager = TunnelManager(config)

    if args.command == "setup":
        manager.setup()
        manager.verify()
    elif args.command == "status":
        active = manager.status()
        print(f"Tunnel: {'✅ active' if active else '❌ inactive'}")
    elif args.command == "verify":
        manager.verify()
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
