import paramiko, os, time

backup_dir = r'C:\Users\HomeUser\vps1'
ssh = paramiko.SSHClient()
ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
ssh.connect('74.208.107.178', username='root', password='k3gaXS7B', timeout=15)

sftp = ssh.open_sftp()
app_dir = os.path.join(backup_dir, 'html')

# These directories are missing from the truncated tar
missing_dirs = ['routes', 'app', 'config', 'database', 'resources', 'tests', 'bootstrap']

for dirname in missing_dirs:
    remote = f'/var/www/html/{dirname}'
    local = os.path.join(app_dir, dirname)
    os.makedirs(local, exist_ok=True)
    print(f'Downloading: {dirname}...')
    try:
        for entry in sftp.listdir_attr(remote):
            remote_file = f'{remote}/{entry.filename}'
            local_file = os.path.join(local, entry.filename)
            if os.path.isfile(local_file) and os.path.getsize(local_file) == entry.st_size:
                continue  # skip if already correct
            import stat
            if stat.S_ISDIR(entry.st_mode):
                os.makedirs(local_file, exist_ok=True)
                for sub in sftp.listdir_attr(remote_file):
                    sf = f'{remote_file}/{sub.filename}'
                    lf = os.path.join(local_file, sub.filename)
                    import stat
                    if stat.S_ISDIR(sub.st_mode):
                        os.makedirs(lf, exist_ok=True)
                        for sub2 in sftp.listdir_attr(sf):
                            s2f = f'{sf}/{sub2.filename}'
                            l2f = os.path.join(lf, sub2.filename)
                            if not os.path.isfile(l2f):
                                try:
                                    sftp.get(s2f, l2f)
                                except: pass
                    else:
                        if not os.path.isfile(lf):
                            try:
                                sftp.get(sf, lf)
                            except: pass
            else:
                try:
                    sftp.get(remote_file, local_file)
                except: pass
        count = sum(len(files) for _, _, files in os.walk(local))
        size = sum(os.path.getsize(os.path.join(r, f)) for r, _, files in os.walk(local) for f in files)
        print(f'  Done: {count} files, {size/1024:.1f} KB')
    except Exception as e:
        print(f'  Error: {e}')

sftp.close()
ssh.close()
print('\nBackup of critical directories complete.')
