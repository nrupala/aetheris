import paramiko, os

ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=15)
sftp = ssh.open_sftp()

backup_dir = r'C:\Users\HomeUser\vps1'
app_dir = os.path.join(backup_dir, 'laravel-app')
os.makedirs(app_dir, exist_ok=True)

remote_base = '/var/www/html'
skip_dirs = {'node_modules', '.git'}

def download_dir(remote_path, local_path):
    os.makedirs(local_path, exist_ok=True)
    for entry in sftp.listdir_attr(remote_path):
        remote_entry = os.path.join(remote_path, entry.filename)
        local_entry = os.path.join(local_path, entry.filename)
        if entry.filename in skip_dirs:
            print(f'Skipping: {entry.filename}')
            continue
        if stat_is_dir(entry):
            print(f'Dir: {entry.filename}')
            download_dir(remote_entry, local_entry)
        else:
            try:
                sftp.get(remote_entry, local_entry)
                print(f'  File: {entry.filename} ({entry.st_size / 1024:.1f} KB)')
            except Exception as e:
                print(f'  FAIL: {entry.filename} ({e})')

def stat_is_dir(attr):
    import stat
    return stat.S_ISDIR(attr.st_mode)

print(f'Downloading {remote_base} to {app_dir}...')
download_dir(remote_base, app_dir)

total_files = 0
total_size = 0
for root, dirs, files in os.walk(app_dir):
    for f in files:
        total_files += 1
        total_size += os.path.getsize(os.path.join(root, f))
print(f'\nTotal: {total_files} files, {total_size / 1024 / 1024 / 1024:.2f} GB')

sftp.close()
ssh.close()
