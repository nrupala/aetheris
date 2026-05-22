import paramiko
import os
import stat

ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=30)

sftp = ssh.open_sftp()
target_dir = r'C:\Users\HomeUser\vps1\laravel-complete'

# Items to download
items = ['app', 'routes', 'config', 'database', 'resources', 'tests', 'bootstrap', 'artisan', '.env', 'composer.json', 'package.json', 'composer.lock', 'webpack.mix.js', 'readme.md']

def download_recursive(remote_path, local_path):
    remote_path = remote_path.replace('\\', '/')
    try:
        attrs = sftp.stat(remote_path)
    except Exception as e:
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
for item in items:
    remote = f'/var/www/html/{item}'
    local = os.path.join(target_dir, item)
    count = download_recursive(remote, local)
    print(f'{item}: {count} files')
    total += count

print(f'\nTotal: {total} files')

sftp.close()
ssh.close()
