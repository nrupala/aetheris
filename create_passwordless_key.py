from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.backends import default_backend

# Generate Private Key
private_key = rsa.generate_private_key(
    public_exponent=65537,
    key_size=2048,
    backend=default_backend()
)

# Save Private Key (NO Password)
pem = private_key.private_bytes(
    encoding=serialization.Encoding.PEM,
    format=serialization.PrivateFormat.TraditionalOpenSSL,
    encryption_algorithm=serialization.NoEncryption()
)

with open("C:\\Users\\HomeUser\\.oci\\aetheris_vm_key.pem", "wb") as f:
    f.write(pem)

# Save Public Key
public_key = private_key.public_key()
ssh_pub = public_key.public_bytes(
    encoding=serialization.Encoding.OpenSSH,
    format=serialization.PublicFormat.OpenSSH
)

pub_key_str = ssh_pub.decode('utf-8') + " aetheris-oracle"

with open("C:\\Users\\HomeUser\\.oci\\aetheris_vm_key.pub", "w") as f:
    f.write(pub_key_str)

print("Public Key Generated (NO Password):")
print(pub_key_str)
