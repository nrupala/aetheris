import oci
import sys

CONFIG_PATH = 'C:\\Users\\HomeUser\\.oci\\config_airwaterclouds'
PROFILE = 'DEFAULT'

# Replace this with your SSH PUBLIC KEY content
# e.g., "ssh-ed25519 AAAA..." or "ssh-rsa AAAA..."
SSH_PUBLIC_KEY = "ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAABAQD/qG1KdNEvVfACegbPEMv26I7kc4M6/Gi8yHMGfevPYQJVMp9bSuERKT9VLiCIStr3uElmNAWBQVHnT8aoEbcrQaKZ4/GxpSsQMqRBgk8hyl0SI42MWIqSHYmgiatM3HJimNRaKG4kRz+SQ7tjTg4qyQVEeYQytNBvnAaubwDfoV9uizfPQWCtKu0+sIvyyGZvERIDz9S3PzR/YqfsdUrFUiCzmwQXmfLhdks/Z8q9OY4Wni24X9W+DSATqasMM0GTqyICiL0krzM3i3gyNfmrrsrd3n4GMi4yx2+FQzLZJeES+YvQlEL4jRf1JCaW76PTg4nX/Xpd1R1lxIwawoA3 aetheris-oracle"

def main():
    print("--- Starting OCI Instance Provisioning ---")
    
    try:
        config = oci.config.from_file(file_location=CONFIG_PATH, profile_name=PROFILE)
        compartment_id = config['tenancy']
        region = config['region']
        
        # Clients
        network_client = oci.core.VirtualNetworkClient(config)
        compute_client = oci.core.ComputeClient(config)

        # 1. Create VCN
        print("\n[1/4] Creating VCN...")
        vcn_response = network_client.create_vcn(
            oci.core.models.CreateVcnDetails(
                compartment_id=compartment_id,
                display_name="Aetheris-VCN",
                cidr_block="10.0.0.0/16",
                dns_label="aetheris"
            )
        )
        vcn = oci.wait_until(
            network_client, 
            network_client.get_vcn(vcn_response.data.id),
            'lifecycle_state', 
            'AVAILABLE'
        ).data
        print(f"VCN created: {vcn.id}")

        # 2. Create Internet Gateway
        print("\n[2/4] Creating Internet Gateway...")
        igw_response = network_client.create_internet_gateway(
            oci.core.models.CreateInternetGatewayDetails(
                compartment_id=compartment_id,
                vcn_id=vcn.id,
                display_name="Aetheris-IGW",
                is_enabled=True
            )
        )
        igw = oci.wait_until(
            network_client, 
            network_client.get_internet_gateway(igw_response.data.id),
            'lifecycle_state', 
            'AVAILABLE'
        ).data
        print(f"Internet Gateway created: {igw.id}")

        # 3. Create Subnet & Route Table
        print("\n[3/4] Creating Subnet...")
        # Get Default Route Table
        route_tables = network_client.list_route_tables(vcn_id=vcn.id, compartment_id=compartment_id).data
        rt_id = route_tables[0].id
        # Update Route Table with IGW
        network_client.update_route_table(
            rt_id,
            oci.core.models.UpdateRouteTableDetails(
                route_rules=[oci.core.models.RouteRule(
                    cidr_block="0.0.0.0/0",
                    network_entity_id=igw.id
                )]
            )
        )
        
        # Default Security List
        security_lists = network_client.list_security_lists(vcn_id=vcn.id, compartment_id=compartment_id).data
        sec_list_id = security_lists[0].id
        # Ensure SSH is allowed
        network_client.update_security_list(
            sec_list_id,
            oci.core.models.UpdateSecurityListDetails(
                ingress_security_rules=[
                    oci.core.models.IngressSecurityRule(
                        protocol="6", # TCP
                        source="0.0.0.0/0",
                        tcp_options=oci.core.models.TcpOptions(
                            destination_port_range=oci.core.models.PortRange(min=22, max=22)
                        )
                    )
                ]
            )
        )

        subnet_response = network_client.create_subnet(
            oci.core.models.CreateSubnetDetails(
                compartment_id=compartment_id,
                vcn_id=vcn.id,
                display_name="Aetheris-Subnet",
                cidr_block="10.0.0.0/24",
                dns_label="aethersub",
                route_table_id=rt_id,
                security_list_ids=[sec_list_id],
                prohibit_public_ip_on_vnic=False
            )
        )
        subnet = oci.wait_until(
            network_client, 
            network_client.get_subnet(subnet_response.data.id),
            'lifecycle_state', 
            'AVAILABLE'
        ).data
        print(f"Subnet created: {subnet.id}")

        # 4. Find Ubuntu 24.04 x86_64 Image
        print("\n[4/5] Finding Ubuntu 24.04 Image...")
        image_response = compute_client.list_images(
            compartment_id=compartment_id,
            operating_system="Canonical Ubuntu",
            operating_system_version="24.04",
            shape="VM.Standard3.Flex"
        )
        # Filter for x86_64 (AMD64)
        # Since 'architecture' attribute might not be consistently named across SDK versions, 
        # we will look for 'Minimal' or just take the first one if shape matches.
        images = [img for img in image_response.data if 'Minimal' in img.display_name]
        if not images:
            images = image_response.data
            
        if not images:
            print("ERROR: Could not find Ubuntu 24.04 AMD64 image.")
            return
        image = images[0]
        print(f"Using Image: {image.display_name}")

        if SSH_PUBLIC_KEY == "REPLACE_WITH_YOUR_SSH_PUBLIC_KEY":
            print("\nWARNING: Please edit this script and paste your SSH PUBLIC KEY in the SSH_PUBLIC_KEY variable.")
            print("   Example: ssh-ed25519 AAAA... your@email.com")
            return

        # 5. Create Instance
        print("\n[5/5] Creating Instance (VM.Standard3.Flex, 1 OCPU, 16GB RAM)...")
        shape_config = oci.core.models.LaunchInstanceShapeConfigDetails(
            ocpus=1,
            memory_in_gbs=16
        )
        create_vnic_details = oci.core.models.CreateVnicDetails(
            subnet_id=subnet.id,
            assign_public_ip=True
        )
        metadata = {"ssh_authorized_keys": SSH_PUBLIC_KEY}
        
        launch_details = oci.core.models.LaunchInstanceDetails(
            availability_domain="oslT:US-CHICAGO-1-AD-1",
            compartment_id=compartment_id,
            shape="VM.Standard3.Flex",
            shape_config=shape_config,
            display_name="aetheris-node-01",
            source_details=oci.core.models.InstanceSourceViaImageDetails(
                image_id=image.id,
                boot_volume_size_in_gbs=50
            ),
            create_vnic_details=create_vnic_details,
            metadata=metadata
        )

        instance_response = compute_client.launch_instance(launch_details)
        
        # Wait for instance to be running
        print("Waiting for instance to boot...")
        instance = oci.wait_until(
            compute_client,
            compute_client.get_instance(instance_response.data.id),
            'lifecycle_state',
            'RUNNING'
        ).data
        
        print("\nSUCCESS! Instance Created:")
        print(f"Name: {instance.display_name}")
        print(f"OCID: {instance.id}")
        
        # Public IP might take a moment to attach, let's fetch it
        vnic_attachments = compute_client.list_vnic_attachments(instance_id=instance.id).data
        if vnic_attachments:
            vnic_id = vnic_attachments[0].vnic_id
            vnic = network_client.get_vnic(vnic_id).data
            print(f"Public IP: {vnic.public_ip}")
            print(f"\nConnect via SSH:")
            print(f"ssh -i <your_private_key> ubuntu@{vnic.public_ip}")
        else:
            print("WARNING: Could not immediately retrieve VNIC. Public IP may take a minute.")

    except Exception as e:
        print(f"\nERROR: {e}")

if __name__ == "__main__":
    main()
