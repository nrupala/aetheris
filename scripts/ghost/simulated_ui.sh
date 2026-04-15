#!/bin/sh
echo "Welcome to Aetheris Restricted Shell (v0.9.1-beta)"
echo "Unauthorized access is logged."

while true; do
    printf "[user@aetheris-ghost ~]$ "
    read cmd
    case $cmd in
        ls) 
            echo "passwords.txt  network_map.json  backups/  confidential_vault_keys.zfs" 
            ;;
        cat*) 
            echo "Permission denied: Accessing $cmd requires Level 4 clearance." 
            ;;
        help) 
            echo "Available commands: ls, cat, help, exit" 
            ;;
        exit) 
            break 
            ;;
        *) 
            echo "sh: $cmd: not found" 
            ;;
    esac
done
