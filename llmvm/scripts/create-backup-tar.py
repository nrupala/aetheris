import paramiko
import time

ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=30)

print('Creating tarball on server...')
stdin, stdout, stderr = ssh.exec_command('cd /var/www && tar czf /tmp/full-laravel-backup.tar.gz html/ && echo "TAR_DONE" && ls -lh /tmp/full-laravel-backup.tar.gz')
start = time.time()
while True:
    if stdout.channel.recv_ready():
        data = stdout.channel.recv(1024).decode()
        print(data, end='')
        if 'TAR_DONE' in data:
            break
    if stderr.channel.recv_ready():
        data = stderr.channel.recv(1024).decode()
        print(data, end='')
    if time.time() - start > 600:
        print('Timeout creating tarball')
        break
    time.sleep(2)

print('Tarball creation complete')
ssh.close()
