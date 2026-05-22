import paramiko
import os
import stat

ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=30)

sftp = ssh.open_sftp()
target_dir = r'C:\Users\HomeUser\vps1\html'

dirs_to_download = ['routes', 'app', 'config', 'database', 'resources', 'tests', 'bootstrap', 'artisan']

def download_recursive(remote_path, local_path):
    # Always use forward slashes for remote (Linux) paths
    remote_path = remote_path.replace('\\', '/')
    try:
        attrs = sftp.stat(remote_path)
    except Exception as e:
        print(f'Cannot stat {remote_path}: {e}')
        return 0
    
    if stat.S_ISDIR(attrs.st_mode):
        os.makedirs(local_path, exist_ok=True)
        count = 0
        for entry in sftp.listdir_attr(remote_path):
            remote_entry = remote_path + '/' + entry.filename
            local_entry = os.path.join(local_path, entry.filename)
            count += download_recursive(remote_entry, local_entry)
        return count
    else:
        os.makedirs(os.path.dirname(local_path), exist_ok=True)
        try:
            sftp.get(remote_path, local_path)
            return 1
        except Exception as e:
            print(f'Failed {remote_path}: {e}')
            return 0

total = 0
for d in dirs_to_download:
    remote = '/var/www/html/' + d
    local = os.path.join(target_dir, d)
    print(f'Downloading: {d}...')
    count = download_recursive(remote, local)
    print(f'  Done: {count} files')
    total += count

print(f'\nTotal: {total} files downloaded')
sftp.close()
ssh.close()
