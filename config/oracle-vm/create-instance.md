# Oracle Cloud VM Creation - A1.Flex 4OCPU/24GB Free Tier

## Instance Details
- **Region**: ca-montreal-1
- **Shape**: VM.Standard.A1.Flex (ARM)
- **OCPUs**: 4
- **Memory**: 24GB  
- **Cost**: 100% FREE (within 3,000 OCPU-hour/month free tier)

## Steps to Create via Web Console

1. **Login to Oracle Cloud Console**
   - URL: https://console.ca-montreal-1.oraclecloud.com
   - Use your Oracle Cloud credentials

2. **Create Compute Instance**
   - Navigation: Compute → Instances → Create Instance
   - Name: `aetheris-ai-server`
   - Placement: Montreal (ca-montreal-1)
   - Availability Domain: AD-1 (default)
   - Capacity type: On-demand

3. **Image and Shape**
   - Image: Ubuntu 22.04 LTS (ARM64)
   - Shape: VM.Standard.A1.Flex
   - OCPU count: 4
   - Memory: 24GB

4. **Networking**
   - Virtual Cloud Network: Create new VCN
   - Subnet: Create new public subnet
   - Public IP: Assign public IP address
   - SSH access: Allow SSH traffic

5. **Add SSH Key**
   - Generate new SSH key pair
   - Download private key (`aetheris-ai-key`)
   - Save private key securely

6. **Boot Volume**
   - Size: 100GB (free tier includes 200GB total)
   - Performance: Balanced (default)
   - Encryption: Oracle-managed keys

7. **Create Instance**
   - Review configuration
   - Click "Create"
   - Wait for instance to provision (5-10 minutes)

## Post-Creation Setup

1. **SSH into instance**:
   ```bash
   ssh -i aetheris-ai-key ubuntu@<public-ip>
   ```

2. **Run setup script**:
   ```bash
   # Upload setup script
   scp -i aetheris-ai-key config/oracle-vm/setup.sh ubuntu@<public-ip>:~
   
   # Make executable and run
   chmod +x setup.sh
   export CLOUDFLARE_TUNNEL_TOKEN="your-tunnel-token-here"
   ./setup.sh
   ```

## Free Tier Validation
- **Total OCPU hours**: 4 OCPU × 750 hours/month = 3,000 OCPU-hours
- **Total GB hours**: 24GB × 750 hours/month = 18,000 GB-hours  
- **Within free tier limits**: ✅ Yes
- **Monthly cost**: $0.00

## Monitoring Usage
- Check usage in Console: Governance → Usage Reports
- Monitor remaining free tier allocation
- Set up budget alerts if needed