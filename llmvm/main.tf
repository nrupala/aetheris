# =============================================================================
# Aetheris LLMVM — Oracle Cloud Infrastructure (Always Free ARM)
# Target: us-chicago-1 (Chicago) or eu-jovanovac-1 (Serbia)
# =============================================================================

terraform {
  required_providers {
    oci = {
      source  = "oracle/oci"
      version = "~> 8.6.0"
    }
  }
}

# -----------------------------------------------------------------------------
# Provider — primary region (set via terraform.tfvars)
# -----------------------------------------------------------------------------

provider "oci" {
  tenancy_ocid     = var.tenancy_ocid
  user_ocid        = var.user_ocid
  fingerprint      = var.fingerprint
  private_key_path = var.private_key_path
  region           = var.region
}

# -----------------------------------------------------------------------------
# Variables
# -----------------------------------------------------------------------------

variable "tenancy_ocid" {
  description = "OCI Tenancy OCID"
  type        = string
}

variable "user_ocid" {
  description = "OCI User OCID (API user)"
  type        = string
}

variable "fingerprint" {
  description = "OCI API Key Fingerprint"
  type        = string
}

variable "private_key_path" {
  description = "Path to OCI API private key (PEM)"
  type        = string
  default     = "~/.oci/oci_api_key.pem"
}

variable "region" {
  description = "OCI Home Region (us-chicago-1 or eu-jovanovac-1)"
  type        = string
}

variable "compartment_ocid" {
  description = "OCI Compartment OCID (root tenancy for new accounts)"
  type        = string
}

variable "ssh_public_key" {
  description = "SSH public key for instance access"
  type        = string
}

variable "ssh_cidr" {
  description = "CIDR block allowed SSH access"
  type        = string
  default     = "0.0.0.0/0"
}

variable "tunnel_domain" {
  description = "Domain for Cloudflare Tunnel"
  type        = string
  default     = "nrupalakolkar.com"
}

variable "tunnel_name" {
  description = "Cloudflare Tunnel name"
  type        = string
  default     = "llmvm-tunnel"
}

variable "ai_bearer_token" {
  description = "Bearer token for AI endpoint access"
  type        = string
  sensitive   = true
}

# -----------------------------------------------------------------------------
# Data Sources
# -----------------------------------------------------------------------------

data "oci_identity_availability_domain" "ad1" {
  compartment_id = var.compartment_ocid
  ad_number      = 1
}

data "oci_identity_availability_domain" "ad2" {
  count          = var.region == "us-chicago-1" ? 1 : 0
  compartment_id = var.compartment_ocid
  ad_number      = 2
}

data "oci_identity_availability_domain" "ad3" {
  count          = var.region == "us-chicago-1" ? 1 : 0
  compartment_id = var.compartment_ocid
  ad_number      = 3
}

data "oci_core_images" "ubuntu_arm" {
  compartment_id           = var.compartment_ocid
  operating_system         = "Canonical Ubuntu"
  operating_system_version = "22.04"
  shape                    = "VM.Standard.A1.Flex"
  sort_by                  = "TIMECREATED"
  sort_order               = "DESC"
}

# -----------------------------------------------------------------------------
# Networking
# -----------------------------------------------------------------------------

resource "oci_core_vcn" "llmvm_vcn" {
  cidr_block     = "10.0.0.0/16"
  compartment_id = var.compartment_ocid
  display_name   = "llmvm-vcn"
  dns_label      = "llmvm"
}

resource "oci_core_internet_gateway" "llmvm_igw" {
  compartment_id = var.compartment_ocid
  display_name   = "llmvm-igw"
  vcn_id         = oci_core_vcn.llmvm_vcn.id
}

resource "oci_core_route_table" "llmvm_rt" {
  compartment_id = var.compartment_ocid
  vcn_id         = oci_core_vcn.llmvm_vcn.id
  display_name   = "llmvm-rt"

  route_rules {
    destination       = "0.0.0.0/0"
    destination_type  = "CIDR_BLOCK"
    network_entity_id = oci_core_internet_gateway.llmvm_igw.id
  }
}

resource "oci_core_security_list" "llmvm_sl" {
  compartment_id = var.compartment_ocid
  vcn_id         = oci_core_vcn.llmvm_vcn.id
  display_name   = "llmvm-sl"

  egress_security_rules {
    destination = "0.0.0.0/0"
    protocol    = "all"
  }

  ingress_security_rules {
    protocol  = "6"
    source    = var.ssh_cidr
    tcp_options {
      max = 22
      min = 22
    }
  }
}

resource "oci_core_subnet" "llmvm_subnet" {
  cidr_block        = "10.0.1.0/24"
  compartment_id    = var.compartment_ocid
  vcn_id            = oci_core_vcn.llmvm_vcn.id
  display_name      = "llmvm-subnet"
  dns_label         = "llmvm"
  route_table_id    = oci_core_route_table.llmvm_rt.id
  security_list_ids = [oci_core_security_list.llmvm_sl.id]
}

# -----------------------------------------------------------------------------
# Compute Instance
# -----------------------------------------------------------------------------

resource "oci_core_instance" "llmvm_instance" {
  compartment_id      = var.compartment_ocid
  display_name        = "llmvm-aetheris"
  availability_domain = data.oci_identity_availability_domain.ad1.name
  shape               = "VM.Standard.A1.Flex"

  shape_config {
    ocpus         = 4
    memory_in_gbs = 24
  }

  source_details {
    source_type             = "image"
    source_id               = data.oci_core_images.ubuntu_arm.images.0.id
    boot_volume_size_in_gbs = 200
  }

  create_vnic_details {
    subnet_id                 = oci_core_subnet.llmvm_subnet.id
    assign_public_ip          = true
    assign_private_dns_record = true
  }

  metadata = {
    ssh_authorized_keys = var.ssh_public_key
  }
}

# -----------------------------------------------------------------------------
# Outputs
# -----------------------------------------------------------------------------

output "region" {
  value = var.region
}

output "availability_domains" {
  value = compact([
    data.oci_identity_availability_domain.ad1.name,
    try(data.oci_identity_availability_domain.ad2[0].name, ""),
    try(data.oci_identity_availability_domain.ad3[0].name, ""),
  ])
}

output "instance_public_ip" {
  description = "Public IP of the LLMVM instance"
  value       = oci_core_instance.llmvm_instance.public_ip
}

output "instance_private_ip" {
  description = "Private IP of the LLMVM instance"
  value       = oci_core_instance.llmvm_instance.private_ip
}

output "ssh_command" {
  description = "SSH command to connect to the instance"
  value       = "ssh -i ${var.private_key_path} ubuntu@${oci_core_instance.llmvm_instance.public_ip}"
}
