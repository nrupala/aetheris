import os
import base64

# Use ssh-keygen but force empty passphrase and capture output to avoid hanging
# We run it in a way that ensures no prompts.
cmd = 'ssh-keygen -t rsa -b 2048 -f "C:\\Users\\HomeUser\\.oci\\aetheris_vm_key" -N "" -q'
os.system(cmd)

# Verify the public key
pub_path = "C:\\Users\\HomeUser\\.oci\\aetheris_vm_key.pub"
if os.path.exists(pub_path):
    with open(pub_path, 'r') as f:
        pub_key = f.read().strip()
    print("SUCCESS! Public Key Generated:")
    print(pub_key)
else:
    print("ERROR: Key generation failed.")
