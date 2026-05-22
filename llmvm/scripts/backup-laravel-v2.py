import paramiko, os, time

backup_dir = r'C:\Users\HomeUser\vps1'
os.makedirs(backup_dir, exist_ok=True)

ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=15)

# Create clean tar excluding symlinks and huge zip files
cmd = (
    "cd /var/www && "
    "find html "
    "-not -path '*/node_modules/*' "
    "-not -path '*/.git/*' "
    "-not -name '*.zip' "
    "-not -path '*/storage/logs/*' "
    "-not -path '*/storage/framework/cache/*' "
    "-not -path '*/storage/framework/sessions/*' "
    "| tar czf /tmp/laravel-app.tar.gz -T - 2>/dev/null && "
    "ls -lh /tmp/laravel-app.tar.gz"
)
print('Creating clean tar...')
stdin, stdout, stderr = ssh.exec_command(cmd)
out = stdout.read().decode()
err = stderr.read().decode()
print('Tar:', out.strip())
if err:
    print('Warnings:', err[:500])

# Download in SFTP
sftp = ssh.open_sftp()
local_path = os.path.join(backup_dir, 'laravel-app.tar.gz')
print('Downloading...')
sftp.get('/tmp/laravel-app.tar.gz', local_path)
size = os.path.getsize(local_path)
print(f'Downloaded: {size / 1024 / 1024:.1f} MB')

# Clean server
ssh.exec_command('rm /tmp/laravel-app.tar.gz')

# Extract locally
import tarfile
extract_dir = os.path.join(backup_dir, 'laravel-app')
os.makedirs(extract_dir, exist_ok=True)
print('Extracting locally...')
with tarfile.open(local_path, 'r:gz') as tar:
    tar.extractall(path=extract_dir)
print(f'Extracted to {extract_dir}')

# Count
total_files = 0
total_size = 0
for root, dirs, files in os.walk(extract_dir):
    total_files += len(files)
    for f in files:
        total_size += os.path.getsize(os.path.join(root, f))
print(f'Total: {total_files} files, {total_size / 1024 / 1024:.1f} MB')

sftp.close()
ssh.close()
