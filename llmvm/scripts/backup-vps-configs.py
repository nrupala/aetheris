import paramiko, os

backup_dir = r'C:\Users\HomeUser\vps1'
ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=15)
sftp = ssh.open_sftp()

# Grab individual important files
files = [
    ('/var/www/html/.env', 'laravel-.env'),
    ('/var/www/html/composer.lock', 'composer.lock'),
    ('/var/www/html/package.json', 'package.json'),
]

for remote, local_name in files:
    try:
        local_path = os.path.join(backup_dir, local_name)
        sftp.get(remote, local_path)
        print(f'OK: {local_name}')
    except Exception as e:
        print(f'MISSING: {remote} ({e})')

# MySQL dump
print('Creating MySQL dump...')
stdin, stdout, stderr = ssh.exec_command('mysqldump --all-databases --routines --triggers | gzip > /tmp/all-db.sql.gz 2>/dev/null && ls -lh /tmp/all-db.sql.gz')
out = stdout.read().decode()
print('Dump size:', out.strip())

db_path = os.path.join(backup_dir, 'all-db.sql.gz')
sftp.get('/tmp/all-db.sql.gz', db_path)
print(f'DB downloaded: {os.path.getsize(db_path) / 1024 / 1024:.1f} MB')

# Cleanup
ssh.exec_command('rm /tmp/all-db.sql.gz')

# List key app files
cmd = "find /var/www/html -maxdepth 3 -type f \\( -name '*.php' -o -name '*.blade.php' -o -name '*.js' -o -name '*.vue' \\) | head -200"
stdin, stdout, stderr = ssh.exec_command(cmd)
print('Key app files:')
print(stdout.read().decode()[:3000])

sftp.close()
ssh.close()
