import oci

CONFIG_PATH = 'C:\\Users\\HomeUser\\.oci\\config_airwaterclouds'
PROFILE = 'DEFAULT'
INSTANCE_OCID = 'ocid1.instance.oc1.us-chicago-1.anxxeljsyu7o5zqcssybgzqlpehq45ilira2vauctj3ce2cqb2htfjzpcoka'

# The NEW public key you want to use
NEW_PUBLIC_KEY = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQC/evxgvQyF7aL/bHXT7/99cmufKV937qInRGVA7vF6Jeq80uXuFCR4XwhEFZ6A4v1BY9fjgiGRLtCIfbF9J8E1n/ONxEil0bucf+Sf5Ar9TW1eUpX0eA0mhdKfFuRaYO7Hmxrf0L2yE7fcgZNIWi442yoQpRKhFa/7l9JyMQuW662lZvHa4vBCtNsBhR6vQRPZsUh3S8nlhMzU2Qx7FL7ry613xc3DHY9j9MHnBgQkQebTCiNskzRHmY0Tdi9X/cELzjlotjpXZhzlo0htDcRHpPj0JmtFbl4p7mZQS7taBhwgqlzxnKycAnrF0/CWhuMTdDXQPxm+nL9zEQ9JMo6F ssh-key-2026-05-04"

def main():
    try:
        config = oci.config.from_file(file_location=CONFIG_PATH, profile_name=PROFILE)
        compartment_id = config['tenancy']
        
        compute_client = oci.core.ComputeClient(config)

        # Update Instance Metadata
        print(f"Updating SSH key on instance {INSTANCE_OCID}...")
        update_details = oci.core.models.UpdateInstanceDetails(
            metadata={"ssh_authorized_keys": NEW_PUBLIC_KEY}
        )
        
        response = compute_client.update_instance(INSTANCE_OCID, update_details)
        print("Metadata update initiated.")
        print("You may need to restart the instance for changes to take effect.")
        print("If you still cannot connect, please restart the instance from the Console.")
        
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    main()