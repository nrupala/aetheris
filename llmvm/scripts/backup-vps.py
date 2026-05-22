import paramiko, os

backup_dir = r'C:\Users\HomeUser\vps1'
os.makedirs(backup_dir, exist_ok=True)

ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=15)

sftp = ssh.open_sftp()

# Download compressed html backup
print('Downloading html-backup.tar.gz (2.5GB)...')
sftp.get('/tmp/html-backup.tar.gz', os.path.join(backup_dir, 'html-backup.tar.gz'))
print('Done: html-backup.tar.gz')

# Dump and download databases
print('Creating DB dump...')
stdin, stdout, stderr = ssh.exec_command('mysqldump --all-databases | gzip > /tmp/all-db.sql.gz 2>&1')
stderr.read()

# Try to get the file
try:
    sftp.get('/tmp/all-db.sql.gz', os.path.join(backup_dir, 'all-db.sql.gz'))
    print('Done: all-db.sql.gz')
except Exception as e:
    print(f'DB download failed: {e}')

# Check MySQL DB sizes
stdin, stdout, stderr = ssh.exec_command('mysql -u root -e "SELECT table_schema, ROUND(SUM(data_length+index_length)/1024/1024,2) AS MB FROM information_schema.tables GROUP BY table_schema;" 2>/dev/null')
print('DB sizes:', stdout.read().decode())

# Disk summary
stdin, stdout, stderr = ssh.exec_command('du -sh /var/www/html /var/lib/mysql /etc 2>/dev/null')
print('Disk usage:', stdout.read().decode())

sftp.close()
ssh.close()
