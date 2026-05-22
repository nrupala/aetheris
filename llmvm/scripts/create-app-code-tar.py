import paramiko
import time

ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=30)

# Create app-code only tarball (no vendor, no public, no storage)
cmd = (
    "cd /var/www/html && "
    "tar czf /tmp/app-code-only.tar.gz "
    "--exclude='public' "
    "--exclude='vendor' "
    "--exclude='storage' "
    "--exclude='node_modules' "
    "html/ 2>&1 && "
    "ls -lh /tmp/app-code-only.tar.gz && "
    "echo APP_CODE_DONE"
)

stdin, stdout, stderr = ssh.exec_command(cmd)

# Read output with timeout
result_lines = []
start = time.time()
while time.time() - start < 120:
    if stdout.channel.recv_ready():
        data = stdout.channel.recv(4096).decode()
        print(data, end='')
        result_lines.append(data)
        if 'APP_CODE_DONE' in data:
            break
    if stderr.channel.recv_ready():
        data = stderr.channel.recv(4096).decode()
        print(data, end='')
    time.sleep(1)

ssh.close()
