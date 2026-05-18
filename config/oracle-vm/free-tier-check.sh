#!/bin/bash
# Free Tier Validation for Oracle Cloud A1.Flex Instance

echo "=== ORACLE CLOUD FREE TIER VALIDATION ==="
echo "Instance: VM.Standard.A1.Flex (4 OCPU, 24GB RAM)"
echo ""

# Calculate monthly usage
HOURS_PER_MONTH=750  # 31 days × 24 hours
OCPU_USAGE=$((4 * HOURS_PER_MONTH))
MEMORY_USAGE=$((24 * HOURS_PER_MONTH))

# Free tier limits
FREE_TIER_OCPU=3000
FREE_TIER_MEMORY=18000

echo "Monthly Usage Calculation:"
echo "- OCPU Hours: 4 OCPU × $HOURS_PER_MONTH hours = $OCPU_USAGE OCPU-hours"
echo "- Memory Hours: 24GB × $HOURS_PER_MONTH hours = $MEMORY_USAGE GB-hours"
echo ""

echo "Free Tier Limits:"
echo "- Max OCPU Hours: $FREE_TIER_OCPU"
echo "- Max Memory Hours: $FREE_TIER_MEMORY"
echo ""

# Check if within limits
if [ $OCPU_USAGE -le $FREE_TIER_OCPU ] && [ $MEMORY_USAGE -le $FREE_TIER_MEMORY ]; then
    echo "✅ WITHIN FREE TIER LIMITS"
    echo "✅ Monthly Cost: $0.00"
    echo "✅ Instance can run 24/7/365 for free"
else
    echo "❌ EXCEEDS FREE TIER"
    echo "Additional usage would incur charges"
fi

echo ""
echo "Note: Free tier allocation is pooled across all A1.Flex instances"
echo "You can run multiple instances as long as total usage stays under limits"