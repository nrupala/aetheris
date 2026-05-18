import oci

config = {
    "user": "ocid1.user.oc1..aaaaaaaa6lhpatjzwz3h7xyqpqgvcowzkthhb3rfzrcrvrzyk76z32frbr7a",
    "fingerprint": "a7:1e:73:01:a3:9b:b4:8b:48:00:9f:ff:5c:35:3f:0f",
    "tenancy": "ocid1.tenancy.oc1..aaaaaaaacx47cqacy3ermkk5o7fej5tik2qxow7ur5p73mm7holahcem3dya",
    "region": "us-chicago-1",
    "key_file": "K:\\LT28178\\.oci\\oci_api_key.pem",
}

client = oci.core.ComputeClient(config)
instances = client.list_instances(compartment_id=config['tenancy']).data

for i in instances:
    # Get detailed instance info
    inst = client.get_instance(i.id).data
    shape_config = inst.shape_config
    
    print(f"Name: {inst.display_name}")
    print(f"  Shape: {inst.shape}")
    print(f"  OCPUs: {shape_config.ocpus}")
    print(f"  Memory: {shape_config.memory_in_gbs} GB")
    print(f"  State: {inst.lifecycle_state}")
    
    vnic_att = client.list_vnic_attachments(
        compartment_id=config['tenancy'],
        instance_id=i.id
    ).data
    if vnic_att:
        net_client = oci.core.VirtualNetworkClient(config)
        vnic = net_client.get_vnic(vnic_att[0].vnic_id).data
        print(f"  Public IP: {vnic.public_ip or '(none)'}")
    print()
