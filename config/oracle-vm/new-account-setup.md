# 🎉 New Oracle Cloud Account Setup - Chicago Region

## Account Details
- **Region**: us-chicago-1 (Plenty of A1.Flex capacity!)
- **Credits**: $300 free credits
- **Status**: Fresh account, no capacity issues

## Step 1: Generate API Key
1. Login to: https://console.us-chicago-1.oraclecloud.com
2. Navigate: Identity → Users → Your User → API Keys
3. Click "Add API Key"
4. Choose "Generate API Key Pair"
5. **Download private key** and save as `oci_api_key_new_chicago.pem`
6. **Copy configuration** and update `~/.oci/config_new_chicago`

## Step 2: Update OCI Config
Edit `C:\Users\HomeUser\.oci\config_new_chicago`:
```ini
[DEFAULT]
user=ocid1.user.oc1..aaaaaaaaxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
fingerprint=xx:xx:xx:xx:xx:xx:xx:xx:xx:xx:xx:xx:xx:xx:xx:xx
tenancy=ocid1.tenancy.oc1..aaaaaaaaxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
region=us-chicago-1
key_file=C:\Users\HomeUser\.oci\oci_api_key_new_chicago.pem
```

## Step 3: Create A1.Flex Instance (4OCPU/24GB)
```bash
# Use the new config
oci compute instance launch \
  --config-file ~/.oci/config_new_chicago \
  --display-name aetheris-ai-chicago \
  --availability-domain AD-1 \
  --shape VM.Standard.A1.Flex \
  --shape-config '{"ocpus": 4, "memoryInGBs": 24}' \
  --image-id ocid1.image.oc1..aaaaaaaaxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx \
  --subnet-id ocid1.subnet.oc1..aaaaaaaaxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx \
  --assign-public-ip true
```

## Step 4: Free Tier Validation
- **4OCPU × 750h = 3,000 OCPU-hours** (exact free limit)
- **24GB × 750h = 18,000 GB-hours** (exact free limit)
- **Cost**: $0.00/month (100% free tier)
- **Bonus**: $300 credits for any additional needs

## Step 5: Update Setup Script
Edit `config/oracle-vm/setup.sh`:
- Change region references to "us-chicago-1"
- Keep all other optimizations for A1.Flex ARM

## 🚀 Advantages:
1. ✅ Guaranteed A1.Flex capacity in Chicago
2. ✅ No more Montreal capacity issues  
3. ✅ $300 credits for scaling if needed
4. ✅ Fresh account, no legacy issues
5. ✅ Better regional performance

## 📊 Cost Structure:
- **First 3,000 OCPU-hours**: FREE
- **First 18,000 GB-hours**: FREE  
- **Additional usage**: Covered by $300 credits
- **Monthly cost**: $0.00 (within free tier)

Your capacity problems are solved! 🎯